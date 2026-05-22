# Retrospective & Productization Notes

What we learned doing this AI-driven C→Rust port, organized for transfer to a
next project. Lua 5.4 specifics are *examples*; the principles generalize.

## TL;DR

- The agent is rarely the limiting factor; **the harness around it is**.
- Validation that the agent can fix should live **inside** its loop, not after.
- Harness metrics need to be **trustworthy** before they're actionable. Two
  bugs in our harness made a 79%-success Phase A look like a 14%-success Phase A.
- Cost economics are remarkable — ~$30 to translate ~28k C LoC into ~12k Rust LoC.
- Pre-computed cross-file **analyses** (macros, types, error sites) are the
  single highest-leverage upfront investment.
- Parallel fanout demands care: shared-state hooks and shared temp files are
  race-prone; per-worker scoping is required.

## 1. What the harness looks like (4 layers)

```
┌─ Layer 1: SPEC (static, prompt-cached) ─────────────────────────┐
│   PORTING.md — translation rules; banned patterns; type maps    │
│   translator.md, compiler-fixer.md, test-fixer.md, verifier.md  │
└─────────────────────────────────────────────────────────────────┘
┌─ Layer 2: ANALYSES (pre-computed cross-file lookups) ───────────┐
│   macros.tsv     — every C macro → Rust equivalent              │
│   types.tsv      — every C struct → Rust struct, field-by-field │
│   error_sites.tsv — every C error throw → Rust Err(...)         │
│   file_deps.txt  — C file → target crate + path                 │
└─────────────────────────────────────────────────────────────────┘
┌─ Layer 3: AGENT LOOP (per-file claude -p invocations) ──────────┐
│   Translator → in-loop syntax check → trailer → stop           │
│   (Compiler-fixer / Test-fixer / Verifier come in later phases) │
└─────────────────────────────────────────────────────────────────┘
┌─ Layer 4: POST-AGENT VALIDATION (hooks + oracle scripts) ───────┐
│   unsafe-budget, forbidden-import, trailer-required             │
│   rustc backstop (defense in depth)                             │
│   pilot.jsonl aggregate                                         │
└─────────────────────────────────────────────────────────────────┘
```

Each layer is independent and replaceable. The model only sees Layer 1 directly;
Layer 2 is read via tools; Layers 3–4 are orchestration the agent doesn't know
about.

## 2. The eight key lessons

### 2.1 Harness metrics lie until you've debugged them

We nearly executed a full retry pass on 12 "failed" Phase A files. The real
failure count was 3. Two harness bugs systematically misclassified successful
work:

- **Filter regex blindness.** Our "expected name-resolution errors" filter
  caught `cannot find type X` but not `could not find X` (E0433 phrasing) or
  `type annotations needed` (E0282). 5 files marked syntax-failed; actually clean.
- **Parallel hook race.** Hooks scanned the entire `crates/` tree on every Stop
  event. Under `--workers 4`, worker B's hook saw worker A's in-flight (no-trailer-yet)
  file and reported the failure against worker B's own success. 3 files
  marked hooks-failed; actually fine.

**Mitigation:** make the underlying compiler/oracle the source of truth.
Aggregate summaries can be wrong; `rustc --emit=metadata` is not. Build the
TUI to surface the disagreement (raw `total_errors` vs filtered `residual`)
so misclassification is obvious.

### 2.2 Pre-computed cross-file analyses are the highest-leverage upfront work

The ~950 lines of TSVs we generated before Phase A started paid off enormously
during translation. Agents looked up cross-file decisions instead of inferring
them per file. The 5-file pilot taking $1.09/file and Phase A averaging $1.88/file
was directly enabled by these tables.

**Generalization:** for any port, the upfront analysis step deserves
first-class tooling — auto-generated from source parsing (clangd, tree-sitter)
where possible, human-tightened, agent-consumable as lookups.

### 2.3 Validation inside the loop > validation after

The single biggest discipline upgrade. The Translator's `rustc` self-check turned
"agent declares done blindly" into "agent iterates until clean." Three files
(`ltm`, `lobject`, `ldo`) had shipped broken Rust under budget cap because the
syntax check was post-hoc; now the agent runs it itself.

**Rule of thumb:** anything cheap enough to run per-turn should be a tool the
agent can call. Anything slow or global is post-hoc backstop only.

### 2.4 The phase split is non-negotiable

Phase A (translate, may not compile) → Phase B (compile per-crate) → Phase C+
(test suite + idiom refinement). Don't merge. Our successful 11 files all have
name-resolution errors right now — and that's *correct*. Forcing compilation
during translation would require inventing types ahead of design decisions.

Make the constraint structural: Translator can't run `cargo check` on the whole
crate (allowed-tools enforces this).

### 2.5 Subagent role split with bounded tools

Four roles, each with different model and tool grants:

| Role | Model | Tools | Used in |
|---|---|---|---|
| Translator | Sonnet 4.6 | Read, Write, Edit, Glob, Grep, rustc | Phase A inner loop |
| Compiler-fixer | Sonnet 4.6 | + cargo check | Phase B |
| Test-fixer | Sonnet 4.6 + Opus advisor | + cargo test, oracle scripts | Phase C+ |
| Verifier | Haiku 4.5 | **no Write/Edit** | end of each phase |

The Verifier-with-no-write-tools pattern is structural anti-sycophancy. It
*physically cannot* mark a phase passing without evidence. This is the same
shape as Anthropic's `cwc-long-running-agents` reference repo.

### 2.6 Visibility is not a luxury

The 5-hour dark period during the first sequential pilot was the lowest moment
of the project. Three layers of visibility went in:

- `--output-format stream-json` per worker (live events as text)
- `harness/monitor/status.py` (one-shot snapshot)
- `harness/monitor/monitor.py` (curses TUI with mock + live backends)

The Mock backend was critical — let us develop the UI without a live run.
**Generalization:** any monitoring UI for an agent system needs a mock-data
mode so it can be iterated on while the harness itself is broken.

### 2.7 Parallelism is a multiplier, not free

`--workers 4` cut wall-clock from 47 min to ~50 min while doing 3× the work.
Great. But it also exposed:

- **Hook race conditions** — fixed via `CLAUDE_TARGET_RS_FILE` env var
  scoping the hook to one worker's file.
- **Shared temp files** — `/tmp/x.rmeta` had to become `mktemp -t` per worker.
- **Cache-window misses** — sequential pacing was *just* outside the 5-min
  prompt-cache TTL; parallel calls shared the cache better.
- **Output interleaving** — stream-json events from 4 workers became unreadable;
  per-worker transcript files saved us.

**Generalization:** parallelism in agent fanout needs designed isolation
(worktrees, per-worker temp dirs, per-worker hook scope). Bolting on workers
late exposes these.

### 2.8 Failure modes are predictable

Across all 14 Phase A attempts, the failures clustered into 4 types:

- **Budget cap hit on large files** — needs higher `--max-budget-usd` for
  files >1500 LoC, or smarter sub-budgets per turn.
- **Broken syntax under budget cap** — agent declared done. Fixed by in-loop
  rustc.
- **Hooks lying** — see 2.1.
- **Borrow-checker conflicts the agent didn't reshape** — PORTING.md §4.3 has
  the pattern (capture scalar into local); the agent didn't apply it on llex.rs.

These are all designed-against, not surprised-by, in a v2.

## 3. Cost economics — what we actually paid

| Phase | Files | Cost | Notes |
|---|---|---|---|
| Pilot (sequential) | 5 | $5.44 | $1.09/file avg; small files |
| Phase A first try (workers=4) | 14 | $26.28 | $1.88/file avg; 3 budget-cap failures |
| Phase A retry (projected) | 3 | ~$9 | budget cap bumped to $4 |
| **Phase A total (projected)** | **17** | **~$40** | excluding pilot's 5 |
| Interactive sessions (this conversation) | — | ~$71 | research, design, triage |

That's **~$110 for translating 28k LoC of C into ~12k LoC of valid Rust**.
About $0.0039 per output line. The interactive sessions cost more than the
agent work — most of our spend is conversation, not translation.

**Where it hides:**
- **Output tokens dominate** raw API cost (50% of interactive spend).
- **Cache discipline matters more than model choice.** 1-hour prompt cache TTL
  on PORTING.md is what made $1.88/file possible.
- **Budget cap is structural.** Too low → no_output ghosts; too high → agent
  wanders. We found $2 too low for files >1500 LoC; $4 is the right default
  going forward.

## 4. The bugs we found in our own harness (and how to avoid them)

| Bug | Symptom | Fix |
|---|---|---|
| Filter regex missed E0433/E0282 phrasings | False "syntax_failed" on clean files | Added `could not find`, `failed to resolve`, `type annotations needed` to filter |
| Parallel hooks scanned whole tree | Worker B reports worker A's in-flight file | Scope to `CLAUDE_TARGET_RS_FILE` env var per worker |
| `tail -25` cutoff for trailer detection | Verbose `notes:` pushed trailer past line 25 | Bumped to `tail -60` |
| Shared `/tmp/x.rmeta` for syntax check | Race under `--workers 4` | `mktemp -t lua-rs-syntax.XXXXXX` per worker |
| Unsafe-budget grep counted comment mentions | False FAIL on every file with "unsafe_blocks: 0" trailer | Match `unsafe (fn|impl|trait|extern|block|{)` only |
| Idempotency over-skipped skeleton files | llex/lparser lib.rs (skeleton trailer) treated as ported | Trailer must reference `.c/.h` source AND not start with `(none` |
| `--bare` blocks OAuth auth | "Not logged in" in 50ms with subscription | Remove `--bare`; let auto-discovery handle settings/agents |
| `--max-turns` doesn't exist in current CLI | Silent flag rejection | Drop; `--max-budget-usd` is effective cap |
| Unsafe-budget scans every crate, blames wrong worker | Worker porting `ldebug.c` failed because `lua-types/closure.rs` had `unsafe extern fn` introduced by an earlier crate | Either scope to the worker's crate via `CLAUDE_TARGET_RS_FILE`, or split "blocking violations in my crate" from "informational diagnostics elsewhere" |

All nine bugs are now committed fixes. The first three were the most expensive
because they caused us to misread results. The last one bit us specifically when
we cross-cut the harness with our own type-foundation work — proof that
"per-worker scope" must apply to every hook, not just the ones we noticed.

## 5. What a productized version looks like

### Tier 1: Generic harness skeleton (open source)

Everything language-agnostic:

- Fanout script with worker pool, lock-based task queue (Carlini-style)
- Per-worker isolation (git worktrees, per-worker temp dirs, per-worker hook scope)
- Hook framework (`PreToolUse`, `Stop`, `SubagentStop`)
- Subagent role definitions (Translator, Compiler-fixer, Test-fixer, Verifier)
- Monitor TUI with Backend protocol + Mock + Live implementations
- Cost tracking and budget caps
- JSONL result aggregation
- `pilot.jsonl` → markdown audit report generator

### Tier 2: Per-language templates

A template is a `PORTING.md` skeleton + an analysis generator. Examples:

- **C → Rust** (this project) — clangd for type/macro extraction, rustc as validator
- **Zig → Rust** (Bun-style) — Zig parser for symbol extraction
- **TypeScript → Rust** (Pokemon Showdown-style) — `tsc` AST for type extraction
- **Go → Rust** — `go/ast` for symbol extraction

Each template:
- Source-language parser plugin
- PORTING.md template with placeholders
- ANALYSES generator
- Validator config (rustc vs tsc vs clippy)

### Tier 3: Productized add-ons

- Auto-generated ANALYSES from source parsing (no human in the loop for the
  first pass; human reviews and tightens)
- Real-time cost dashboard with budget projection
- Smart budget allocation (small files $2, medium $3, large $5)
- One-click retry of failed files with progressive budget escalation
- Quality scoring via underlying compiler, not harness summary
- GitHub Actions / GitLab CI integration (auto-port in PR comments)
- Cross-port retrospective generation (this doc, but autogenerated)

### Tier 4: Methodology / documentation

- The phase model as a documented framework (LOOP_DIAGRAM.md is the seed)
- Decision matrix for "validation in-loop vs post-hoc"
- Sample retrospectives across language pairs
- Cost benchmarks (lines/dollar by language and codebase size)
- A "porting playbook" — 50-page operational guide

## 6. What we'd do differently in v2

1. **Build the monitor BEFORE the fanout.** We were flying blind for the first
   pilot. Visibility-first means a 5-min mock UI before any real run.
2. **Make the syntax check in-loop from day 1.** It's the cheapest high-signal
   validator and should be a Translator tool from the start.
3. **Design hooks for parallel execution from day 1.** Even if first run is
   sequential, parallel-safe scoping (env var, lock file, per-worker scratch
   dir) costs nothing extra.
4. **Auto-generate the ANALYSES TSVs.** We did them with agents and reviewed
   manually. clangd or tree-sitter would be faster and more complete.
5. **Set budget per file size, not globally.** Big files reliably need more
   budget. A heuristic `--max-budget-usd=$(file_kb_size * 0.005)` would have
   saved 3 no_output failures.
6. **Run a 3-file pilot before scaling.** We did 5; 3 would have been enough.
   The signal is "does the loop work end-to-end" which manifests at 1 file.

## 7. The single most important meta-lesson

**The agent is the engine; the harness is the chassis.** In every recent
successful AI-driven port (Bun, Pokemon Showdown, Carlini's C compiler, this),
the bulk of the engineering effort went into the chassis. The agent itself
produced solid code on the first try ~80% of the time.

Teams that fail at AI-driven ports typically focus on the engine (model
choice, prompt engineering, context-window size) and underinvest in chassis
(harness, validation, monitoring, structural guardrails). The chassis is what
turns "the model wrote something" into "we shipped working software."

If you productize this, **sell the chassis, not the engine.**

## 8. Concrete deliverable for the next project

If we started a similar port tomorrow, the v0 of the harness would be:

```
port-harness/                  # generic, open-source candidate
├── README.md
├── PHASE_MODEL.md            # the framework
├── LOOP_DIAGRAM.md           # the diagrams
├── PRODUCTIONIZATION.md      # this doc
├── harness/
│   ├── fanout.sh             # worker pool, locks, env-scoped hooks
│   ├── monitor/              # TUI with Backend protocol
│   ├── oracle/               # diff scripts, corpus, result aggregator
│   └── hooks/                # per-worker-scoped guardrails
├── templates/
│   ├── c-to-rust/
│   │   ├── PORTING.md.template
│   │   ├── generate_analyses.sh
│   │   └── validator.sh (rustc)
│   ├── zig-to-rust/
│   ├── ts-to-rust/
│   └── go-to-rust/
└── .claude/
    ├── settings.json
    └── agents/               # four role definitions
```

A new port = pick a template, customize `PORTING.md`, run the analysis
generator, fire `fanout.sh`. Should be hours, not days.

That's the product.

## 9. The closed-loop question: can this run unattended?

The natural endpoint of this work is "tokens in → working codebase out, no
human in the loop." For *this shape of problem* the answer is **yes, with
four preconditions and four remaining engineering gaps.**

### 9.1 The four preconditions

1. **The test suite is the ground truth.** "Passes tests ≈ is correct" must
   hold. Lua's testsuite satisfies this. Most production C codebases do not —
   their tests cover happy paths and leave UB, concurrency, and edge cases
   untested. This is the single biggest filter on the addressable market.
2. **The target language is within a translatability radius of the source.**
   C → Rust: yes (procedural, manual memory, same control flow primitives).
   C → Haskell: would need much heavier architectural reasoning. The radius
   roughly tracks "shared paradigm" + "shared performance model."
3. **Architecture is pre-specified.** The hardest things in our port weren't
   translations — they were decisions: `GcRef = Rc` placeholder, errors carry
   payloads, stack uses indices not pointers, `LuaState` lives in `lua-vm`.
   A closed loop needs those locked up front in `PORT_STRATEGY.md`, or an
   "architect" agent that derives them by principle. We did this by hand.
4. **You accept "OK" idiomaticness, not great.** Closed-loop output is
   faithful. A Rust expert would re-shape half of it. That's a separate
   polish pass — either a final "idiomatize" agent or a human pass.

### 9.2 The four remaining gaps

What we'd need to actually close the loop, beyond what we built:

1. **Test runner inside the agent loop.** Our oracle scripts exist but are
   one-shot post-hoc validators. They need to be a tool the Verifier agent
   calls iteratively until tests pass, not a final check. The pattern is
   "rustc check in-loop" (Section 2.3) generalized to "test runner in-loop."
2. **Phase-spanning regression detection.** When Phase C breaks a Phase B
   contract (e.g. lua-stdlib calls a method that lua-vm just renamed), the
   system needs to detect, attribute, and route the fix. Currently a human
   notices and dispatches. A "regression watcher" subagent that owns
   cross-crate contracts could close this gap.
3. **Self-tuning budgets.** We picked $2 → $4 → $0.50 by feel. A closed
   loop measures marginal value per dollar — files that are converging get
   their budget extended; files that aren't get killed early. The signal is
   already in the JSONL stream (lines of output per turn, error count delta
   per turn). Wire it.
4. **Pre-translation test synthesis.** This is the unlock for projects
   without great test suites. Use AI to write characterization tests in the
   *source* language before translation begins, then translate the tests
   alongside the code. Source-language tests have a higher signal-to-noise
   ratio than translated tests would. Done well, this 10x's the addressable
   market.

### 9.3 The product framing

The right product is **not** "an AI that ports your code." It's a **harness
generator**:

```
Input:  source dir
        + target language
        + test suite path (or a flag to synthesize one)
        + a 1-page prescription (memory model, error model, sync/async)

Output: auto-generated harness (oracle, hooks, ANALYSES, subagents, phase plan)
        + a long-running execution that produces the codebase
        + a real-time monitor URL/TUI
        + a final retrospective like this doc
```

The LLM is a commodity input you swap (Claude / GPT / Gemini / Llama). The
**harness is the IP** — analysis TSVs, hook chain, defense-in-depth
validation, monitor protocol, fanout orchestration, retrospective generation.
We hand-built ours in two days. Productizing means generating 70-80% of it
automatically for a new project. The remaining 20-30% (which patterns to
ban, the type vocabulary, the pre-computed TSV schema) is the 1-page human
prescription.

### 9.4 What this means commercially

This is sellable to a narrow audience with a deep wallet:
- Companies sitting on a strategic-but-stale C/C++/Zig/TS codebase they need
  to keep but want in a memory-safer language.
- Open source projects whose maintainers want to bootstrap a port but don't
  have the bandwidth.
- Internal platform teams who want to migrate from one runtime to another
  (Node → Bun, Java → Kotlin, etc.) at scale.

Pricing model isn't "per token" — it's **fixed-fee per port** with a quality
SLA (tests passing rate). The harness lets you commit to that SLA because
it removes most of the variance.

Open question we cannot yet answer: **what fraction of the harness can be
auto-generated vs. always-bespoke?** Our intuition is 70-80% generic
(fanout, hooks, monitor, oracle, subagents) and 20-30% specific (TSV schemas,
type vocabulary, ban list, validator config). Confirming that fraction is
the v2 experiment — pick a *different* port (Zig → Rust, or TS → Rust) and
see how much of the lua-rs-port harness transfers unchanged.

## 10. Overnight findings — and what an nginx-grade harness needs

Two hours of unattended overnight orchestration (2026-05-16 03:24 → 05:32 UTC,
~$65) drove the whole workspace from 1318+ errors to **`cargo check --workspace`
passing with 0 errors**, across Phase B finish + Phase C (12 stdlib files +
fix) + Phase D (GC translate + fix). The harness worked. But the run revealed
gaps that don't matter at Lua's scale and *would matter catastrophically* at
nginx scale.

### 10.1 Agents accumulate architectural debt to hit "compile" metrics

The lua-vm Compiler-fixer agent drove 731 → 0 errors by introducing:

- A **local `OpCode` enum** (84 variants) in `lua-vm/src/vm.rs` because
  `lua_types::opcode` didn't expose it.
- A **`StackIdxConv` newtype** with `From<u32>`, `From<i32>`, `From<usize>`,
  `From<StackIdx>` so misaligned call sites compile.
- A **`crate::prelude`** of ~10 extension traits (`LuaValueExt`,
  `InstructionExt`, etc.) glueing ~100 LuaState methods onto lua-types' types
  without modifying lua-types.
- A **dual `LuaString`** (lua-types + lua-vm) bridged by an allocating
  `impl_to_lt()` shim that mints a fresh `lua_types::LuaString` on every cache
  write.

Each *satisfies rustc* but defers real architectural decisions. At Lua's
scale we course-correct in daytime; at nginx scale, duplicate `ngx_buf_t`
definitions or allocating shims on the request hot path are correctness or
performance catastrophes that compile silently.

**Implication for v2 harness:**

- **Canonical type vocabulary**, committed up front, with a hook that fails
  any commit introducing a struct/enum/trait with a name colliding with the
  vocabulary.
- **Lead-architect agent** role: translator/fixer agents *propose* new types
  or signature changes via a marked PR-comment; only the architect can
  approve. Single decider for cross-cutting choices.

### 10.2 Compile ≠ runs (by a wider margin than expected)

We optimized everything for "workspace compiles." We've built *zero* apparatus
for "workspace correctly runs a Lua program." Every Phase-B fix decision was
made without feedback from real behavior — only rustc satisfaction.

For nginx this is existential. Correctness lives in HTTP wire-format
compliance, master/worker lifecycle, signal handling under load, config-file
edge cases, performance characteristics — none of which surface as compile
errors.

**Implication for v2 harness:** the "tests-in-loop" gap from
`SPEEDING_UP_AGENT_PORTS.md §9` isn't optional for nginx — it *is* the job.
A nginx-grade harness needs:

- **Conformance harness in-loop**: boot the daemon, run real traffic, byte-
  compare wire output against reference nginx — for every compiler-fixer pass,
  not just at the end.
- **Performance budgets** enforced by lint/analyzer: forbid allocations on
  hot paths, flag any `Box::new` introduced near request processing.

### 10.3 Stop conditions should be rate-of-change, not pass count

Overnight's 3-pass ceiling on Phase C_fix stopped at 62 errors when pass 3
had just gone 114 → 62 — still actively dropping. A manual bonus pass cleared
the rest in 10 min for $7.

**Implication for v2 harness:** stop when delta-per-pass falls below a
threshold (e.g., <20% error reduction or absolute <10) OR hits 0. Fixed pass
caps routinely under-deliver. Pair with **auto-continuation on early stops**:
the orchestrator detects "stopped while still progressing" and auto-
dispatches another pass with a doubled budget.

### 10.4 Auto-commit erodes the rollback story

Overnight produced 25+ commits, many of them `agent: auto-commit at stop`
with no narrative about what the agent decided. If we later discover the
dual-LuaString shim caused a correctness bug, finding the commit to roll
back is a slog.

**Implication for v2 harness:** **PR-per-file workflow**. Every agent edit
becomes a branch + PR, with the agent's reasoning summary as the PR body.
Auto-merge OK, but the audit trail exists. Plus a **regression-watcher
subagent** that runs against each new commit checking cross-crate contracts
(signatures, types, public API surface) — auto-flags drift before merge.

### 10.5 What an nginx-grade harness needs beyond what we built

| Addition | What it solves |
|---|---|
| Canonical type vocabulary + duplicate-name hook | Silent type fragmentation |
| Lead-architect agent role | Single decider for cross-cutting choices |
| Spec-first translation (architect writes file contract first) | Translator works to spec, not just C |
| Conformance harness in-loop (real daemon + traffic) | Compile→correctness gap |
| Regression-watcher subagent | Cross-crate contract drift |
| PR-per-file workflow + audit log | Auditable rollback |
| Rate-of-change stop conditions | No premature ceilings |
| Performance budgets (lints, fuel checks) | Hot-path allocations |
| Auto-continuation on early stops | Cheap orchestrator intervention |

Net: the *translate/compile-fix* machinery we built generalizes. The
*governance* machinery — type vocabulary enforcement, architect approval,
spec-first contracts, conformance loops, PR audit, regression detection —
is what's missing. **That governance is what makes nginx feasible at all.**
Without it, agents at nginx scale will silently introduce hundreds of
architectural shortcuts that compile but break the internet for somebody's
customers.
