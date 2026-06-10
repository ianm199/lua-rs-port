# lua-gc — the garbage collector

A tri-color incremental mark-and-sweep (`src/heap.rs`): roots traced, gray
propagated, white objects freed. Carries budgeted `unsafe` (raw `*mut` for the
object graph) — every block needs `// SAFETY:` and must stay under the ceiling in
`harness/unsafe-budgets.toml`. Read the root `../../CLAUDE.md` first.

## Active frontier (issues #104, #113)

Generational mode is being built on top of the incremental engine. Two live
caveats — know them before you touch GC:

- **#104 — byte accounting is partly simulated.** `gc.lua` passes the behavioral
  oracle, but several of its memory assertions (`collectgarbage('count')`,
  `gcinfo`, `totalbytes`) are satisfied by *formulas*, not real per-allocation
  tracking (see the Phase-B shims in `lua-vm/src/api.rs`). So a green `gc.lua` is
  **not** evidence the accounting is faithful. Phase D replaces these with real
  byte tracking.
- **#113 — generational major-step pacing regressed alloc-heavy workloads ~2×**
  (bisected to the pacing commit). The collector can over-trigger full collections
  under sustained allocation. If you touch `generational_step`/`stepgenfull`,
  re-run the benchmarks (`harness/bench/compare.sh --workloads binarytrees,
  string_ops_long`) and compare the ratio — a GC change that doesn't move the
  trace but doubles `binarytrees` is this bug class.

## Write barriers

Barriers keep the incremental/generational invariant (no black→white pointer
without re-graying). `barrier_back` grays a black parent that receives a white
child. **Every heap store from the VM/stdlib that can put a young object into an
old/black container must be barriered.** A missing barrier is silent until the
collector frees a still-reachable object — the worst failure mode here.

## Verify GC changes against the canaries first

```bash
harness/canaries/gc/run_canaries.sh     # incremental + generational modes
```
These are the fast, deterministic in-memory testers (the CLAUDE.md "custom
subsystem tester" pattern) — run them on **any** GC, metamethod, or table change
before the slow `gc.lua` oracle. Then `harness/run_official_test.sh
reference/lua-c/testes/gc.lua` and `gengc` for the full oracle.

## Rooting bugs: quarantine mode is the inner loop

`LUA_RS_GC_QUARANTINE=1` (debug build) makes any use-after-sweep dereference
panic with a backtrace instead of silently reading freed memory: sweep parks
dead boxes on a poisoned quarantine list, `Gc::as_box`/`Marker::mark_box`
assert. Cadence is identical to a normal run. Pair with `LUA_RS_GC_STRESS=1`
to make cadence-dependent anchoring bugs deterministic. The full battery is
`harness/asan-stress.sh` (issue #140, `docs/EXACT_ROOTING_SPEC.md`); debug
builds also assert that a coroutine mutably borrowed at collect time is
covered by a parent snapshot (`trace_reachable_threads`).

## Plan
`specs/followup/issue-93-generational-gc-plan.md` is the living design for the
generational work.
