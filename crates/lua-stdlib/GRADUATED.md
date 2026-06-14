# Graduated: lua-stdlib

Per-module graduation declarations for the standard library
(Idiomatization Sprint 2 — stdlib / Phase 2). Plan of record:
`docs/IDIOMATIZATION_ROADMAP.md`, `docs/IDIOMATIZATION_SPRINT_2_SPEC.md`.

Phase 2 is different from Sprint 1 in one decisive way: **there is no structural
(bytecode-parity) oracle.** stdlib emits behavior, not bytecode. The only net is
the behavioral suite, and it is coarser. The discipline each module follows is:
strengthen the behavioral net FIRST, idiomatize ONLY what the net covers, and
leave net-uncovered algorithm code LOAD-BEARING (extract/rename, never refactor).

---

## math

Status: graduated 2026-06-14 (Sprint 2, Phase P2a — the first Phase-2 module,
the first subsystem with no structural oracle). Branch of record: `idiom/math`.

### What "graduated" means here

`crates/lua-stdlib/src/math_lib.rs` was originally a line-by-line port of
PUC-Rio `lmathlib.c`. As of P2a the C correspondence is intentionally gone for
the idiomatized surface: the 8 `PORT NOTE` blocks, the `lmathlib.c:NNN` /
C-internals references, the `LUAMOD_API → pub` / `I2UInt` macro-correspondence
notes, the stale `to_integer_opt` `TODO`, and the dead Phase-A scaffolding (the
`MATHLIB` static + `LibReg` struct, ~120 lines that the live registration path
never used) have all been removed. The `PORT STATUS` trailer was condensed to
current state and now points at the behavioral net rather than at a C line count.

Do **not** open `lmathlib.c` to reason about the idiomatized functions — but DO
keep reading the inline comments that remain: they describe **behavior**
(per-version gates and the subnormal bit-math), not C structure, and they were
KEPT on purpose (see "Left load-bearing" below).

### The oracle that now guards it (behavioral-only — no bytecode parity)

A change to math is verified only when all are green (the P2a gate):

1. **Behavioral suite.** `cargo test -p omnilua --test multiversion_oracle`
   (169 — the prior 165 plus the four net-strengthening assertions added FIRST in
   P2a), the full official suite (`harness/run_official_all.sh`, math.lua PASS),
   and the version-gated batteries (`specs/oracle/check.sh 5.1`..`5.5` at the
   baseline 57/54/23/7/10, 0 fail).
2. **The crate's own fast net, new in P2a:** `cargo test -p lua-stdlib --test
   math_float_only` (2 white-box tests) — the first test module in this crate.
   It reaches through the `omnilua` `Value` enum (a dev-dependency) at the raw
   `Value::Integer` vs `Value::Number` distinction, which Lua's own `type()`
   **cannot observe** under the 5.1/5.2 float-only model.
3. **Crate gates:** `cargo test --workspace`, `cargo check -p lua-stdlib
   --target wasm32-unknown-unknown`. `unsafe` blocks: 0 (unchanged).

#### The net had to be STRENGTHENED before idiomatizing — the Phase-2 story

This is the key Phase-2 methodology output. At baseline the net was **strong for
pure algebra but gappy for the PRNG / subnormals / float-only invariants**, so
the FIRST three commits added tests (each proven non-tautological by mutation,
each green against the un-idiomatized baseline):

- **PRNG sequence (the #1 gap).** The old oracle pinned ONE draw per seed, so a
  2nd-call or projection-path divergence would pass undetected. Added multi-call
  no-reseed sequence pins (float draws, `random(lo,hi)` positive and signed
  ranges, the two-seed-word `randomseed(n1,n2)` form) on **5.4 and 5.5** — the
  only bit-exact path. (5.1/5.2/5.3 wrap host C `rand()`, a documented allowed
  divergence; see "Honest-negative" below.)
- **FloatOnly type invariant.** Under 5.1/5.2 every `math.random` result must be
  a `Float`, never an `Int` — invisible to `type()`. The white-box test above
  pins it; a mutation that pushed `Int` was caught (`Integer(8)`).
- **Subnormal ldexp/frexp.** Promoted the subnormal edges (`ldexp(1.0,-1074)`,
  underflow-to-+0.0, smallest normal, overflow-to-inf, `frexp` of the smallest
  subnormal/normal incl. sign) into the standard gate, on every version that
  exposes the functions (5.3/5.4/5.5). A naive `x * 2f64.powi(e)` was caught
  (yields `0.0`).

### Left load-bearing — extract/rename only, NEVER refactored (the net does not cover these)

- **The xoshiro256\*\* PRNG core** (`next_rand`, `rand_to_float`, `project`,
  `set_seed_words`) — now grouped, byte-for-byte unchanged, into a private
  `mod xoshiro`. The sequence is pinned bit-exact, so ANY reordering of the
  arithmetic diverges; the module doc says so explicitly. The grouping is the
  ONLY change — no algorithm line moved.
- **`ldexp` bit-scaling and `frexp` mantissa/exponent split.** Subnormal-correct
  by construction (bounded power-of-two chunking; the subnormal-input scale-up
  branch in frexp). A naive simplification loses subnormals. The inline comments
  explaining the bit-math were KEPT.
- **ALL version gates — NOT consolidated.** `is_v53`, `float_only`, the
  `empty_arg` index, the compat-math roster (`atan2`/`cosh`/`sinh`/`tanh`/`pow`/
  `log10`, 5.1–5.4 only), `frexp`/`ldexp` surviving into 5.5, `math.log`'s
  base-arg 5.1-vs-5.2+ split, `math.mod` 5.1 alias, the 5.1/5.2 nil-registration
  of `type`/`tointeger`/`ult`/`maxinteger`/`mininteger`, and the `randomseed`
  return-count / require-seed / auto-seed splits are each checked **per version
  in isolation** by the oracle. Collapsing them can silently break a
  never-construct-`Int` or wrong-arg-index invariant. The inline comments on each
  gate describe behavior (oracle-verified against the reference binaries), not C.

### Honest-negative recorded in this graduation

The PRNG **sequence** is bit-exact only on the xoshiro256\*\* path (5.4/5.5). On
5.1/5.2/5.3 the reference wraps the host C `rand()`/`random()`, whose byte stream
is platform-dependent — a KNOWN, DOCUMENTED allowed divergence
(`specs/followup/5.1-numbers-prng.md`, `specs/research/5.3-upstream-delta.md`).
The spec asked for "at least one of 5.1/5.3" sequence pins; the faithful answer
is that neither is pinnable to the reference (pinning our own output would be
tautological, which the phase forbids), so their **contract** (range/type/shape/
arg-error) is pinned instead and the **sequence** is intentionally not. This is
the right reason to STOP short on that blind spot, not a gap in coverage.

### Recipes harvested

See the "Recipe ledger" → "### P2a — math" in
`docs/IDIOMATIZATION_SPRINT_2_SPEC.md`.
