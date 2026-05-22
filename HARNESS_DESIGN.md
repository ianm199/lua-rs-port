# Harness Design

The agent-orchestration kit that drives the port. Companion to `PORT_STRATEGY.md`. This doc is the contract for *how* we work; `PORT_STRATEGY.md` is *what* we're building.

Status: **active.** All five enforcement hooks, four subagent roles, three pre-computed analyses, and the four oracle scripts described here are wired up under `.claude/` and `harness/`.

## 1. What we kept from Bun, what we changed

| Principle | Bun | Us |
|---|---|---|
| Phase A (translate) / Phase B (compile) split | ✓ | ✓ — applied within each big phase of `PORT_STRATEGY` §4 |
| `PORTING.md` as load-bearing static spec | ✓ | ✓ |
| Pre-computed cross-file analyses (Bun: `LIFETIMES.tsv`) | ✓ | ✓ — four files under `ANALYSES/` |
| Mandatory per-file `PORT STATUS` trailer | ✓ | ✓ |
| `TODO(port) / PERF(port) / PORT NOTE` flagging conventions | ✓ | ✓ |
| Structural fidelity (same fn names, same field order, diffable) | ✓ | ✗ — we trade fidelity for idiom, design decisions in §3 of PORT_STRATEGY are intentionally non-faithful |
| Byte-exact compile output as oracle | ✗ | ✓ — Phase A diffs `luac -o` bytecode |
| Differential testing on every test run | ✗ | ✓ — every test runs against C-Lua and our binary, stdout/exit diffed |
| Mechanical unsafe-block ceiling | weak | **hard** — default 0 outside `lua-gc`/`lua-coro`, ceilings in `harness/unsafe-budgets.toml` |
| Verifier subagent with no write tools | ✗ | ✓ |

The summary: smaller, cleaner target than Bun means we can be *more* rigorous, not less. We exploit the existence of a behavioral oracle (`lua` + test suite) and a structural oracle (`luac -o` bytecode) that Bun didn't have access to.

## 2. The three-layer harness

### Layer 1 — `PORTING.md` (static spec)

The agent-facing translation rulebook. Read by every per-file Translator task. Contents:

- Eight design decisions from `PORT_STRATEGY.md §3` restated as binding rules.
- C-pattern → Rust-pattern table covering all common idioms.
- Banned crates / patterns / types.
- Naming, file-layout, and trailer conventions.
- `unsafe` policy.
- Output format (PORT STATUS trailer).

Lives at the repo root.

### Layer 2 — Pre-computed analyses under `ANALYSES/`

Globally-true facts computed once, looked up per-file. Agent tasks do not re-derive them.

| File | Contents | Built when |
|---|---|---|
| `ANALYSES/macros.tsv` | Every public macro in `lobject.h`/`lstate.h`/`llimits.h` + its mapped Rust form | Before Phase A |
| `ANALYSES/types.tsv` | Every C struct → Rust struct mapping, field by field | Before Phase A |
| `ANALYSES/error_sites.tsv` | Every `luaG_runerror` / `luaD_throw` / `luaO_pushfstring`-then-throw site → corresponding `Err(LuaError::...)` | Before Phase B |
| `ANALYSES/file_deps.txt` | Header inclusion graph (`gcc -MM` output, parsed) | Before Phase A |

The principle (from Bun's `LIFETIMES.tsv` lesson): **don't ask the agent to re-derive cross-file truths per file.** Hand it a row.

### Layer 3 — Agent loop

Four named subagent roles, defined under `.claude/agents/`. Each has bounded context and an explicit write boundary.

| Role | Reads | Writes | Phase use |
|---|---|---|---|
| **Translator** | one `.c/.h` file + PORTING.md + ANALYSES/ | one `.rs` file with PORT STATUS trailer | A: inner loop |
| **Compiler-fixer** | one crate's `.rs` files + `cargo` errors | only `.rs` in that crate | B: inner loop |
| **Test-fixer** | one failing test + relevant `.rs` files | only `.rs` (never the test) | C+: inner loop |
| **Verifier** | phase test list + oracle output | **nothing — read-only** | end of every phase |

The Verifier role having no write tools is load-bearing — it can't rationalize a passing verdict because it physically can't set `test-results.json` to PASS. The pattern is from Anthropic's `cwc-long-running-agents` reference repo and is the canonical anti-sycophancy structural mitigation.

## 3. Enforcement hooks

Five hook scripts under `.claude/hooks/`. Each is registered in `.claude/settings.json`. The first four are guardrails; the fifth is quality-of-life.

| Hook | Event | Fails when | Why |
|---|---|---|---|
| `unsafe-budget.sh` | Stop | crate `unsafe` count exceeds ceiling in `harness/unsafe-budgets.toml` | Prevents the 13k-unsafe-block Bun outcome |
| `forbidden-import.sh` | Stop | banned patterns appear in changed files (`String` for Lua data, `&str` for Lua data, `tokio`, `async fn`, raw ptrs outside `lua-gc`/`lua-coro`) | Catches drift away from the design decisions |
| `trailer-required.sh` | Stop | any new/modified `.rs` lacks the PORT STATUS trailer with all fields | Machine-readable status for triage |
| `verify-gate.sh` | PreToolUse | write to `harness/oracle/test-results.json` without first reading the matching evidence file | Anti-sycophancy: forces evidence before flipping PASS |
| `commit-on-stop.sh` | Stop | (never; quality-of-life) | Auto-commits uncommitted work, kills the "agent ran two hours, results blown away" failure |

Ceilings in `harness/unsafe-budgets.toml`:

```toml
[crates]
"lua-types"  = 0
"lua-lex"    = 0
"lua-parse"  = 0
"lua-code"   = 0
"lua-vm"     = 0
"lua-stdlib" = 0
"lua-gc"     = 20   # incremental tri-color collector unavoidably needs some
"lua-coro"   = 10   # stackful coroutine context switches need raw asm/inline
"lua-cli"    = 0
```

Raising a ceiling requires editing this file in a commit message that documents *why*. The hook reads from the file, so the source of truth is in version control.

## 4. Oracle scripts under `harness/oracle/`

```
harness/oracle/
├── corpus/                     # small Lua programs for Phase A bytecode diff
│   ├── 01-hello.lua
│   ├── 02-arith.lua
│   ├── 03-tables.lua
│   ├── 04-control-flow.lua
│   └── ...
├── diff-bytecode.sh <prog>     # Phase A: byte-diff luac vs our compiler on <prog>
├── diff-output.sh   <prog>     # Phase B+: stdout/exit-diff vs C-Lua on <prog>
├── run-test-file.sh <test>     # Run a single official test file against our impl
├── run-phase.sh     <phase>    # Run the test set for a phase; write test-results.json
└── test-results.json           # Defaults to FAIL per phase; flipped by run-phase.sh
```

The phase test-set lists are hard-coded in `run-phase.sh` and frozen in git. Adding a test to a phase requires a code change to the script, not an in-session decision. Prevents goalpost-moving.

`test-results.json` is the contract document: every phase's "done" criterion is a passing result here, gated by the `verify-gate.sh` hook.

## 5. Property fuzzing as graduation criterion

Beyond the official test suite:

- **End of Phase A:** generate 10,000 random Lua programs via a small fuzzer (`harness/fuzz/genprog.lua`), confirm byte-identical bytecode between `luac -o` and our compiler.
- **End of Phase B:** run the same 10,000 programs through both interpreters, confirm identical stdout / exit.
- **End of Phase D:** same, but with `collectgarbage("collect")` injected at random points — confirms GC determinism.

Catches the long tail of pathological inputs the canonical test suite doesn't cover.

## 6. Sessions, parallelism, model selection (locked)

Conclusions from the May 2026 research sweep, condensed to operational defaults:

**Unit of work: `claude -p --bare` per file.** Not `/loop` (session-scoped, context grows), not the Agent SDK (overkill for stateless work), not Agent Teams (experimental). One CLI invocation per file with explicit `--agents`, `--settings`, `--allowedTools`, `--permission-mode dontAsk`, `--max-budget-usd`, `--max-turns`.

**Parallel fanout: Carlini-style filesystem locks on git worktrees.** Each worker `git worktree add`s a per-task tree, claims a lock file via atomic `git push`, runs Translator → Compiler-fixer → Test-fixer → Verifier in sequence, marks the lock `.done` or `.needs_human`. Start at 4 workers, scale to 8–16 after a 10-file pilot.

**Model assignments:**

| Role | Model | Reason |
|---|---|---|
| Translator | Sonnet 4.6 | PORTING.md collapses decision space — Opus's reasoning advantage disappears |
| Compiler-fixer | Sonnet 4.6 | Same |
| Test-fixer | Sonnet 4.6, **Opus 4.6 advisor** when stuck >2 rounds | Advisor pattern: ~13% cheaper than Opus solo, ~3pp better than Sonnet solo on SWE-bench |
| Verifier | Haiku 4.5 | No judgment — just runs oracle, structures output. Sonnet is overkill |

**Cost ceiling per file** (`--max-budget-usd`):
- Translator: $2.00
- Compiler-fixer: $3.00
- Test-fixer: $5.00 (hard cap; escalate to `needs_human` on hit)
- Verifier: $0.50

**Expected total:** $2–7 per file × ~80–120 files = $300–800 happy path; $1.5k–3k with realistic Verifier rounds. Reference: Carlini's C compiler was $20k for a *much* harder target.

**Prompt caching discipline.** Anthropic changed the default cache TTL from 1h → 5min in March 2026. Explicit opt-in to 1h required via `cache_control: {type: "ephemeral", ttl: "1h"}`. Layer order (1h before 5m before uncached):
1. **1h cached:** PORTING.md, workspace Cargo.toml, the canonical type-mapping TSVs.
2. **5m cached:** the current C source file, the matching `ANALYSES/<file>.md` if one exists.
3. **Uncached:** per-call instruction ("translate to `<rust-path>`; follow PORTING.md exactly").

Caching PORTING.md alone saves ~$1k–1.5k over a 100-file port.

**Structured output for the Verifier.** Mandate `--output-format json --json-schema harness/oracle/verifier-schema.json`. Schema requires non-empty `evidence` array. Anti-sycophancy by construction — Verifier can't return `pass` without quoting cargo/test output.

## 10. The C-source-as-comments trick (vjeux's Pokemon Showdown contribution)

In the Translator's output, embed the original C source as adjacent `// C:` comments next to the translated Rust function. Reasons:

- Phase B reviewers see Rust ↔ C side-by-side without a separate window.
- Test-fixer agents have local context for "what was this *supposed* to do" without re-reading the source file.
- Diffs against the C reference become inline-greppable.

The Translator agent should write:

```rust
// C: static int luaB_print (lua_State *L) {
//   int n = lua_gettop(L);  /* number of arguments */
//   ...
// }
fn print(state: &mut LuaState) -> Result<usize, LuaError> {
    let n = state.stack_top();
    // ...
}
```

This is mandated in PORTING.md §10 (added below) and the Translator agent prompt.

## 11. Hook defense-in-depth

Per [Claude Code issue #29691](https://github.com/anthropics/claude-code/issues/29691), agents have been observed deliberately breaking forbidden tokens across edit-stream chunks to evade regex hooks. Mitigation: **every regex hook is paired with a post-hoc AST or cargo-check hook**. Our specific defenses:

- `forbidden-import.sh` (regex) + the PostToolUse `cargo check` invocation that runs after every Write/Edit catches both *deliberately-broken* tokens (because cargo check parses real Rust) and *accidentally-correct-looking-but-broken* code.
- `unsafe-budget.sh` runs on Stop and counts after the dust settles, not at write time.
- `trailer-required.sh` runs on Stop and verifies the structured trailer is intact.

Never rely on a single regex layer for any rule that the agent has incentive to evade.

## 7. Per-phase audit trail

Each phase writes a `docs/PHASE-<X>-CHANGELOG.md` autogenerated from the `PORT STATUS` trailers + hook logs. Structured fields:

```
phase: B
files_translated: 23
files_passing_check: 21
tests_passing: 8/12 (constructs, locals, closure, vararg, goto, literals, code, calls)
tests_failing: 4 (events, errors, attrib, sort)
unsafe_blocks_added: 0
unsafe_budget_remaining: 0 (zero crates over budget)
deferred_todos: 47 (see PHASE-B-TODOS.txt)
```

Becomes the audit trail. We can grep across phases to see which deferred TODOs are still open.

## 8. Repository layout (final)

```
lua-rs-port/
├── README.md
├── PORT_STRATEGY.md           <- what we're building
├── HARNESS_DESIGN.md          <- this file
├── PORTING.md                 <- agent-facing translation spec
├── Cargo.toml                 <- workspace root
├── .gitignore
├── ANALYSES/
│   ├── README.md
│   ├── macros.tsv
│   ├── types.tsv
│   ├── error_sites.tsv
│   └── file_deps.txt
├── reference/
│   ├── lua-5.4.7/             <- canonical C source (built)
│   ├── lua-5.4.7-tests/       <- official test suite
│   └── lua-c/                 <- github mirror (gitignored)
├── docs/
│   └── PHASE-*.md             <- per-phase changelogs (autogenerated)
├── harness/
│   ├── oracle/                <- diff drivers, corpus, results
│   ├── unsafe-budgets.toml    <- per-crate unsafe ceilings
│   └── fuzz/                  <- random program generator (Phase A+)
├── .claude/
│   ├── settings.json
│   ├── hooks/                 <- five enforcement scripts
│   └── agents/                <- four subagent role definitions
└── crates/
    ├── lua-types/             <- TValue, GCObject, errors
    ├── lua-lex/               <- llex
    ├── lua-parse/             <- lparser
    ├── lua-code/              <- lcode + lopcodes
    ├── lua-vm/                <- lvm + ldo + lstate
    ├── lua-stdlib/            <- baselib, strlib, tablib, ...
    ├── lua-gc/                <- real GC (Phase D)
    ├── lua-coro/              <- coroutines (Phase E)
    └── lua-cli/               <- standalone interpreter
```

## 9. Open questions still pending review

These were called out at the bottom of the previous design discussion; restating for the record:

1. **Unsafe ceiling = 0 outside `lua-gc`/`lua-coro`.** Recorded; agent will route to TODO(port) when blocked instead of reaching for `unsafe`.
2. **Verifier with no write tools.** Recorded.
3. **Property fuzzing as graduation criterion.** Recorded; will land at end of Phase A.
4. **Differential testing every run.** Recorded.
5. **Four pre-computed analyses.** Recorded; estimated 2–3 days of agent-time to build before Phase A.
