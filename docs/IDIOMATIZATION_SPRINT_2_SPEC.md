# Idiomatization Sprint 2 — stdlib (Phase 2): live spec + recipe ledger

Owner: Fable (supervisor: design sign-off + verification). Execution: Opus
subagents per module. Plan of record: `IDIOMATIZATION_ROADMAP.md` Phase 2 + the
**Phase-2 GO** in `IDIOMATIZATION_REFLECTION_1.md §7`. Reuses Sprint 1's
recipe-catalogue format + graduation-declaration template
(`IDIOMATIZATION_SPRINT_1_SPEC.md` → "Phase-0 scaffolding").

## The shift from Sprint 1: the structural oracle is GONE

Sprint 1 (lexer/parser) had **bytecode parity** — a near-total, bisectable net
that survived idiomatizing the producer. stdlib emits *behavior*, not bytecode.
**The only net is the behavioral suite, and it is coarser.** This sprint's whole
discipline is managing that loss. The governing rule (from the reflection):

> Before idiomatizing any stdlib module, verify its behavioral coverage is
> strong enough to stand alone. A module with thin coverage has a weak net;
> **strengthening the net (adding oracle tests) is the FIRST transformation, not
> an afterthought.** Code the net does NOT cover (algorithm-exact PRNG, subnormal
> bit-math) is treated as load-bearing: EXTRACT/rename, never refactor.

## The Phase-2 gate (behavioral-only)

A module is idiomatized only when ALL are green:

1. **Coverage precondition (do FIRST, before any idiomatization).** Produce a
   per-public-function map: WELL-COVERED / WEAKLY-COVERED / UNCOVERED by the
   behavioral net (`run_official_all` module file + `multiversion_oracle` +
   `check.sh` ×5). For every WEAK/UNCOVERED function that idiomatization will
   touch, **add the missing test to the standard gate first** (a sequence test, a
   type-invariant test, an edge-input test). Land the net-strengthening tests as
   their own commit(s) so they are seen to FAIL-then-PASS only against the
   reference behavior — never tautological.
2. **Behavioral suite green:** the module's official test file PASS (via
   `run_official_all`), `multiversion_oracle` (current 165, plus the new
   net-strengthening assertions), `check.sh 5.1`..`5.5` at baseline.
3. **Crate gates:** `cargo test -p lua-stdlib`, `cargo test --workspace`,
   `cargo check --target wasm32-unknown-unknown`.
4. **Perf arbiter — hot modules ONLY.** Cold modules (math, table, os date/time)
   need no arbiter. The string-pattern matcher IS hot → it carries the Ir +
   cold-machine-wall arbiter (`docs/MEASUREMENT_PROTOCOL.md`, the T5a lesson);
   an idiomatization that regresses CPI is a no-go even if behavior is identical.

Load-bearing invariants preserved byte-identical, same as Sprint 1: all version
gates, exact error wording, no public-API change, `unsafe` reduced-never-added.

## Recipe / graduation format

Unchanged from Sprint 1 (`IDIOMATIZATION_SPRINT_1_SPEC.md`). Each module appends
recipes to this doc's "Recipe ledger", a verdict to the "Verdict ledger", and
gets a `crates/lua-stdlib/<module>.GRADUATED.md` (or a shared
`lua-stdlib/GRADUATED.md` with a per-module section) stating which behavioral
net now guards it and which algorithm code was left load-bearing.

## Checklist (tick only with evidence)

- [x] P2.0: scaffolding — this spec (Phase-2 gate + coverage-precondition rule);
      math coverage-precondition verdict recorded below from the 2026-06-14 recon
- [x] P2a: MATH (`math_lib`) — net-strengthened FIRST (PRNG sequence + FloatOnly
      + subnormal, all proven non-tautological and green at baseline), then the
      well-covered pure functions idiomatized; PRNG/ldexp/frexp/version-gates left
      load-bearing; behavioral suite green (oracle 169, math_float_only 2,
      official suite incl. math.lua, check.sh 5.1-5.5 0-fail, workspace, wasm,
      unsafe 0); recipes + verdict below; graduation in
      `crates/lua-stdlib/GRADUATED.md`. Branch `idiom/math` (supervisor PRs).
- [ ] (then, as gates + budget allow: `table` (sort/nextvar nets), `os` date/time
      arithmetic — both cold/pure)
- [ ] (LAST, separate, with the perf arbiter: the `string` pattern matcher)
- [ ] CLOSE: PRs merged CI-green; board row updated

## P2a — math: coverage-precondition verdict (recon 2026-06-14)

The behavioral net is **strong for pure algebra, gappy for PRNG / subnormals /
version-gate invariants.** The packet must STRENGTHEN before it idiomatizes.

**WELL-COVERED → SAFE to idiomatize** (arg-type dispatch helpers, naming,
const-naming the bit-pattern magic numbers, crutch removal): `abs`, `sin`/`cos`/
`tan`/`asin`/`acos`/`atan`, `sqrt`, `exp`, `log`, `deg`/`rad`, `floor`/`ceil`,
`modf`, `min`/`max`, `ult`, `type`, `tointeger`, `huge`/`pi`/`maxinteger`/
`mininteger`.

**WEAK / UNCOVERED → net-strengthen FIRST, then leave the algorithm
LOAD-BEARING (extract/rename only, do NOT refactor the math):**
- `math.random`/`randomseed`: the oracle pins ONE seed point — it would not catch
  a 2nd-call sequence divergence. **Add a multi-call no-reseed sequence assertion
  (per version where the PRNG differs — 5.4/5.5 xoshiro256\*\*) + a FloatOnly
  invariant test** (under 5.1/5.2 every `math.random` result must be `Float`,
  never `Int` — invisible to `type()` but a real invariant). Then do not touch
  `next_rand`/`project`.
- `ldexp`/`frexp`: subnormal edges (`ldexp(1.0,-1074)==5e-324`) are only in
  `multiversion_oracle`, not `math.lua`. **Promote the subnormal assertions into
  the standard gate.** Then leave the bit-scaling untouched.
- Version gates (`is_v53`, `float_only`, `empty_arg`, the compat-math roster,
  `math.log` base arg, `math.mod` 5.1 alias, the 5.1/5.2 nil-registration of
  5.3+ helpers): each is checked per-version in isolation, so **do NOT
  consolidate** — collapsing them can silently break a never-construct-Int or
  wrong-arg-index invariant. Keep them explicit `if` branches.

**Known adjacent bug (NOT in scope, note it):** `math.fmod(x,0)` error omits the
function name (`bad argument #2 (zero)` vs `... to 'fmod' (zero)`) — a shared-core
arg-naming gap, tracked separately; do not "fix" it inside this packet.

## Recipe ledger
(append transformation recipes here as modules graduate)

### P2a — math (`math_lib`), 2026-06-14

`crates/lua-stdlib/src/math_lib.rs` is a ~1043-line port of `lmathlib.c`. The
defining Phase-2 lesson landed here: **the net had to be strengthened before the
code could be safely touched.** The behavioral suite was strong for pure algebra
but had three real holes (PRNG sequence, the float-only Int invariant, subnormal
edges) — so the first three commits were `test(math): ...`, each proven
non-tautological by mutation and green against the un-idiomatized baseline,
*before* any `idiom(math): ...` commit. That ordering is the deliverable.

---

**Recipe: `strengthen the behavioral net FIRST` (the Phase-2 precondition in
practice)**
- Pattern: a stdlib module has no structural oracle; some of its behavior is
  pinned only thinly (one sample) or not at all (an invariant `type()` can't see).
  Idiomatizing against that thin net is the one thing Phase 2 forbids.
- Action, in order, as their own commits before touching the module:
  1. **Find the holes by category:** sampled-not-sequenced (PRNG), invisible-to-
     the-language invariants (float-only never-construct-`Int`), and
     buried-edge (subnormal ldexp/frexp pinned in only one place).
  2. **Capture REFERENCE behavior** from the version-suffixed ref binaries
     (`/tmp/lua-refs/bin/lua5.x`) — never the impl's own output (tautological).
     For a sequence, seed once and pin N consecutive draws; for an invisible
     invariant, reach through a lower-level view than the language exposes; for a
     buried edge, promote it into the standard gate so it runs every time.
  3. **Prove each test real by mutation:** break the thing it guards, watch it
     FAIL, restore, watch it PASS. (Here: corrupt a sequence digit; force the
     float-only push to `Int`; swap `ldexp` for naive `x*2f64.powi(e)`.)
  4. **Confirm green at the un-idiomatized baseline** — that proves the net is a
     real net, not a description of the change you're about to make.
- Invariant that replaced the (absent) structural one: oracle 169 (was 165) +
  `cargo test -p lua-stdlib --test math_float_only` (2) + math.lua + check.sh×5.
- Caveat: where a blind spot CANNOT be strengthened against the reference, STOP
  and record an honest-negative — do **not** pin the impl's own output to make the
  number go up. (Here: the 5.1/5.2/5.3 PRNG sequence wraps host C `rand()` — a
  documented platform-dependent divergence; only 5.4/5.5 xoshiro is bit-pinnable.)

**Recipe: `white-box test for an invariant the language can't observe`**
- Pattern: an invariant is real (the reference upholds it) but invisible through
  the language's own introspection — e.g. under 5.1/5.2 `math.random` must yield a
  `Float` not an `Int`, yet those versions have no `math.type` and `type()` says
  `"number"` for both.
- Action: place the test in the consuming crate but reach one layer below the
  language. Here a `tests/` file in `lua-stdlib` takes `omnilua` as a
  **dev-dependency** (omnilua depends on lua-stdlib, so a normal dep would cycle —
  a dev-dep does not, since it is outside the build graph) and inspects the raw
  `Value::Integer` vs `Value::Number` returned by `eval`. Add a CONTRAST case from
  a version where the subtype IS the other one (5.4 `random(5,8)` → `Integer`) so
  the test is proven to exercise the gate, not a coincidental absence.
- Invariant that now guards it: `v51_v52_random_results_are_always_float` +
  `v54_random_interval_is_integer_subtype`.
- Caveat: this is the only way to net a float-only Int invariant; the behavioral
  oracle alone is blind to it. Keep the contrast case or a future change could
  make BOTH versions wrong and still pass.

**Recipe: `name the recurring arg-type dispatch / push helper`**
- Pattern: the same `if matches!(state.value_at(N), LuaValue::Int(_))` int-vs-float
  branch recurs at many sites; a "push Int when it fits exactly, else Float"
  helper carries inline-commented magic bounds.
- Action: extract `arg_is_int(state, n)` (7 call sites → reads as intent);
  rename `push_num_int` → `push_int_or_float` and lift its `i64::MIN as f64` /
  `-(i64::MIN as f64)` bounds into `const I64_MIN_AS_F64` /
  `I64_MAX_PLUS_1_AS_F64` with a doc-comment explaining the half-open test
  (`i64::MAX` is not exactly representable in `f64`). Also name the
  frame-relative `set_top(state, 1)` "return the arg unchanged" idiom as
  `keep_first_arg`, folding the triplicated relative-vs-absolute inline comment
  into one doc.
- Invariant that guards it: pure renames/extractions — the whole behavioral net
  (oracle 169, math.lua, check.sh×5) is unmoved.
- Caveat: net was already STRONG for these pure paths (abs/floor/ceil/modf/type
  are well-covered) — no strengthening needed first; this is the easy half.

**Recipe: `group the load-bearing algorithm into a named private module`**
- Pattern: a cluster of pure functions IS the load-bearing core (here xoshiro256\*\*
  `next_rand`/`rand_to_float`/`project`/`set_seed_words`, pinned bit-exact). You
  want to make "do not refactor this" structurally obvious without touching it.
- Action: wrap them (plus only the constants they use) in a private `mod xoshiro`,
  `pub(super)`, callers reach them as `xoshiro::*`. **Move/group ONLY — every
  function body byte-identical, nothing reordered.** The module doc states the
  contract ("pinned by the sequence tests; do not reorder the arithmetic").
- Invariant that guards it: the PRNG-sequence pins (strengthened in this packet)
  — they are the tripwire that proves the grouping changed no arithmetic.
- Caveat: this is the Phase-2 form of "leave load-bearing." Do it only AFTER the
  sequence net exists; without the tripwire, grouping a bit-exact algorithm is
  unverifiable. Distinguish from P1's "extract for readability" — here the extract
  is to *fence off* code the coarse net cannot fully prove.

**Recipe: `crutch removal on a behavioral-only module`**
- Same as Sprint 1's recipe, with a Phase-2 keep-list twist. Removed: 8 `PORT
  NOTE`s, the `lmathlib.c`/`LUAMOD_API`/`I2UInt` C-correspondence notes, a stale
  `to_integer_opt` `TODO` (the signature it predicted already exists — verified by
  grep, the parser-packet discipline), ~120 lines of dead `MATHLIB`/`LibReg`
  Phase-A scaffolding the live `MATHLIB_FUNCS` path never used, and the
  C-line-count `PORT STATUS` trailer (condensed to point at the net).
- **Keep-list, sharper here because the net is coarser:** EVERY version-gate
  comment (each gate is oracle-checked per-version in isolation — a collapsed gate
  silently breaks a never-construct-`Int` or wrong-arg-index invariant) and the
  ldexp/frexp subnormal bit-math comments. Distinguish *"this is what the C did"*
  (delete) from *"this is the per-version behavior / the subnormal correctness
  reason"* (keep). One genuine `TODO` survives (the thread-local→per-state PRNG
  migration) — kept because it is real deferred behavior, not stale scaffolding.
- Caveat: comment-only but gate it like code (a blank line detaches a `///`).

## Verdict ledger
(append per-module outcomes — graduated OR honest-negative-with-reason)

### P2a — math: GRADUATED (2026-06-14)

Graduated. Net strengthened first (3 `test(math)` commits, all non-tautological
and green at baseline), then 6 `idiom(math)` transformations, each gated
behavioral-green. Final state: oracle 169, `lua-stdlib` `math_float_only` 2,
official suite incl. math.lua PASS, check.sh 5.1-5.5 at baseline 57/54/23/7/10
(0 fail), workspace green, wasm check OK, **unsafe blocks 0**. Graduation doc:
`crates/lua-stdlib/GRADUATED.md` "math". Branch `idiom/math` (supervisor verifies
+ PRs).

**Honest-negative (within an otherwise-graduated module):** the spec asked for a
seeded-sequence pin on "at least one of 5.1/5.3 (the older PRNG path)." Neither is
pinnable to the reference: 5.1/5.2/5.3 wrap the host C `rand()`/`random()`, whose
byte stream is platform-dependent — a KNOWN, DOCUMENTED allowed divergence
(`specs/followup/5.1-numbers-prng.md`, `specs/research/5.3-upstream-delta.md`;
re-confirmed empirically here: our 5.3 output uses xoshiro and diverges from the
5.3.6 reference for every tested seed). Pinning our own 5.3 output would be
tautological — the one move this phase forbids. Resolution: bit-pin the xoshiro
path (5.4 + 5.5) where it IS exact, and pin the 5.1/5.2/5.3 **contract**
(range/type/shape/arg-error) rather than the sequence. This is a correct STOP on
a blind spot, not a coverage gap.

**Out of scope, noted not fixed:** `math.fmod(x,0)` error omits the function name
(`bad argument #2 (zero)` vs `... to 'fmod' (zero)`) — a shared-core arg-naming
gap tracked separately.
