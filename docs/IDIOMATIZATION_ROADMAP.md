# Literal port → idiomatic Rust — a multi-phase, oracle-gated roadmap

Status: proposed 2026-06-13. Pilot: omniLua (lua-rs). The durable deliverable is
the **methodology** (recipes + the verification-model shift + the graduation
process), extracted to `../port-harness/` once proven — the same way the
perf-packet template was. The prettier Lua code is the by-product.

## Why this is a distinct transformation worth a methodology

The harness has proven transformation #1: **C → faithful safe Rust**, gated by a
*dual* oracle (behavioral output parity + line-by-line structural fidelity to the
C source). lua-rs ships; that loop works.

Transformation #2 — **literal port → idiomatic Rust, incrementally, without
breaking it** — is undocumented anywhere and is arguably more broadly useful:
far more teams own an ugly-but-working Rust port (or any crufty-but-working
codebase) than own a C codebase mid-port. It is the universal "now make it nice
without breaking it" problem. lua-rs is the ideal pilot for developing it:
finished, green, with a strong oracle — exactly the conditions that made it ideal
for transformation #1.

This is **Stage 2 of the port lifecycle**, after the existing Phase A/B/C
(translate → compile → behavior) have produced a correct faithful port.

## The governing principle: idiomatize in descending order of oracle strength

The insight that orders the whole roadmap. Three oracle strengths are in play,
and they are not equal:

1. **Bytecode-parity (structural, strongest).** For the lexer/parser/codegen:
   the output bytecode must equal `luac -l -l`'s, byte for byte. Crucially this
   oracle **survives idiomatizing the producer** — you can rewrite the lexer's
   internals however you like; if the token stream still yields identical
   bytecode, you are provably behavior-preserving. Strongest possible net.
2. **Behavioral suite (output parity).** For stdlib + runtime behavior: the
   official suites + the multiversion oracle. Strong but coarser — it catches
   wrong *outputs*, not internal corruption that happens to not surface.
3. **GC-correctness battery (quarantine + canaries + stress + ASAN).** For the
   collector. The strongest net *available* for the GC, but the GC is also where
   bugs are subtlest — #189 was a rooting hole the behavioral oracle missed on
   almost every collection schedule.

**Rule: idiomatize strongest-net-first.** This is both safest *and* it lets you
develop the recipe catalogue and build confidence on provably-safe ground before
you reach the GC, where the net is thinnest. The corollary maturity gate: **you
have earned the right to idiomatize a subsystem only when its behavioral
verification is strong enough to stand without the C correspondence** — because
idiomatization deliberately destroys the structural oracle (that is the *point*;
you are moving away from the C shape), so you lose the "what does `lgc.c` do
here?" debugging crutch that found #140/#189.

## What "idiomatic" means here — and what it explicitly does NOT

IS idiomatization (mostly needs no `unsafe`):
- iterators / slices over manual index arithmetic
- `Result` + `?` over C-style status codes and out-params
- enums, newtypes, typestate over tag-and-union and naked integers
- RAII / `Drop` over manual cleanup calls (#189's `Error` was a first taste)
- ownership and safe references over raw `NonNull` where not perf-critical
- deleting the `// PORT NOTE: lvm.c:NNN` crutches as a subsystem graduates

IS NOT idiomatization (a different axis — perf-via-unsafe/nightly, out of scope):
- NaN-boxing the value representation (unsafe)
- computed-goto / tail-call dispatch (language-constrained)
These are performance representation changes; do not conflate them with making
the code idiomatic.

The hot dispatch loop (`vm::execute`) is the **explicit exception**: a large
`match` over opcodes *is* the idiomatic Rust shape for a fast interpreter, its
C-correspondence is load-bearing, and the T5a episode showed changes there
regress CPI unpredictably. Leave it largely alone.

## The phases (for lua-rs)

### Phase 0 — methodology scaffolding (small, do first)
Define the reusable machinery before touching code:
- The **recipe-catalogue** format: each idiomatization records a before/after
  transformation pattern ("C char-pointer scan → `Peekable<Chars>`", "int error
  code → `Result` + `?`", "tagged union → enum + methods", "manual free →
  `Drop`") with the behavioral invariant that *replaced* the structural one.
- The **graduation declaration** per subsystem: states which oracle now guards it
  (so a future debugger knows the C correspondence is gone and what to trust
  instead), and the "done-idiomatic" definition + stopping rule for that
  subsystem.
- The per-packet gate template (which oracle tier, plus perf arbiter if hot).
Pick the Phase-1 pilot (the lexer — see below).

### Phase 1 — strong-oracle pilots (bytecode-parity net): lexer → parser → codegen
The lexer (`lua-lex`, port of `llex.c`) is the ideal **first pilot**: cold (a
tiny fraction of runtime), self-contained, and double-gated by bytecode parity
(token stream → identical bytecode) plus the lexical-error/line-number
behavioral tests. Idiomatize it fully (peekable iterator scanning, token enums,
`Result`), drop the PORT NOTEs, write the first recipe entries + its graduation
doc. Then parser (`lparser.c`) and codegen (`lua-code`) under the same
bytecode-parity net. These three develop the catalogue under maximum safety.

### Phase 2 — behavioral-oracle breadth: stdlib modules
`lua-stdlib` modules, behaviorally oracle-gated. Easy first (math, os, table),
then the gnarly + perf-sensitive string-pattern matcher (`match_pat` et al.) —
which also carries the Ir/wall arbiter gate, because it is hot. This broadens the
catalogue and trains the behavioral-only discipline (no structural net).

### Phase 3 — cross-cutting idiom: error handling fully Rust-native
#189 nudged this (the `Error` wrapper). Finish it tree-wide: typed errors, `?`
throughout, `Drop`-based cleanup, retiring the C status-code shape internally.
Cross-cutting, so it trains "refactor across the whole tree while staying green."
Public embedding API stays semver-stable.

### Phase 4 — the marquee: the GC memory model
Re-express `lua-gc`'s intrusive raw-`NonNull` linked lists as something
Rust-native (generational-index arena + handles, or a safe abstraction layer over
the intrusive structure). Research-grade, the headline proof, and the richest
methodology — rewriting the riskiest subsystem with only the behavioral oracle +
the full GC battery (quarantine + stress + canaries + ASAN) as the net. This is
only sane *because* that battery exists, which is itself the lesson: build the
behavioral verification strong enough first, then you have earned the GC rewrite.
Multi-month; the one place idiomatization may surface in the public API (flag and
semver it).

### Phase 5 — extract & generalize (the actual product)
The recipe catalogue + the verification-model-shift doc + the graduation process
graduate to `../port-harness/` as a reusable Stage-2 capability. Once the
catalogue has enough worked recipes, routine idiomatizations route to cheaper
models with the oracle as the net (per `MODEL_ROUTING.md`) — the methodology
teaching itself. Carry it to redis-rs-port and, eventually, the literal nginx
port (which will also be ugly and want this exact transformation).

## Risks (named from this session's lessons)
- **Hot-path idiomatization can regress CPI** (T5a): any change to a hot
  subsystem carries the Ir + cold-machine-wall arbiter, not just the behavioral
  oracle. Ir-down is necessary but not sufficient.
- **The behavioral oracle misses timing-dependent GC bugs** (#189): Phase 4
  requires the stress+quarantine harness, and even then the GC is the riskiest
  phase — treat every rooting-adjacent change as guilty until the battery clears.
- **Losing the C crutch raises debugging cost**: each graduation doc must record
  what behavioral invariant replaced the structural one, or future fixes get
  harder.
- **Scope creep**: "make it idiomatic" is infinite. Each subsystem needs a
  "done-idiomatic" definition and a stopping rule. The goal is the *methodology*
  plus the GC proof, not 100% line coverage.

## Where this sits in the broader project
This is a **parallel methodology track on lua-rs** — muscle-building for a new
harness capability, in the same spirit as the original port. It is internal
(except where the GC surfaces), so it does **not** block the launch follow-through
or churn the public API. It competes with redis/harness-extraction for attention;
the justification is the methodology + content value ("how you actually go from
literal port to idiomatic Rust, multi-phase"), and that the nginx endgame will
eventually want exactly this transformation on its own literal port.

## The first concrete move
**Phase 1 pilot — idiomatize the lexer**, gated by bytecode parity (output
unchanged) + the lexical-error/line-number behavioral tests, producing the first
recipe-catalogue entries and the graduation-doc template. Same shape as the first
perf packet: bounded, oracle-gated, a worked example that proves the loop before
committing to the multi-phase arc.
