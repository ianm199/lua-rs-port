# Issue #93: Generational GC Completion Plan

## Current patch: default-mode parity only

This patch fixes the observable startup-mode half of #93 for hosted runtimes:
Lua 5.4 and Lua 5.5 now start in reported generational mode after the standard
libraries are opened. That mirrors upstream standalone startup:

- raw `lua_newstate` initializes the collector as incremental;
- standalone/runtime startup restarts the collector and switches to
  generational mode before user code runs.

The patch deliberately does not change raw `new_state()` semantics and does not
claim that the generational collector is implemented.

## Why #93 is still architectural

The current `gengc.lua` gate is too weak as proof. `reference/lua-c/testes/gengc.lua`
guards the meaningful age/color assertions behind `T`/`testC`, and returns early
when `T == nil`. Passing that file without `T` only proves the public API does
not crash in the exercised paths; it does not prove real young/old collection.

Current lua-rs scaffolding:

- `GcKind::{Incremental, Generational}` and public
  `collectgarbage("generational"|"incremental")` mode switching exist.
- `genminormul`/`genmajormul` are stored.
- A generational step still runs the same incremental step underneath, then
  performs a mark-only weak-table prune.
- `keep_invariant()` and `is_sweep_phase()` are still hardcoded false.
- VM write barriers are mostly no-op shims.
- Finalizers are queued/drained outside the collector state machine.
- Lua-visible byte accounting still has Phase-B simulation behavior, tracked as
  #104.

So #93 is two layers:

1. default reported mode by version, fixed by the current patch;
2. actual generational collection, still open.

## Required sequencing

### 0. Keep the scoped startup fix

Deliverables:

- `lua-vm::api::configure_startup_gc_mode` switches hosted 5.4/5.5 runtimes to
  generational mode after `open_libs`.
- CLI and embedding/runtime constructors call it.
- Multiversion regression verifies:
  `collectgarbage("incremental")` returns `"generational"` first, then mode
  switches round-trip as reference does.

Verification:

- `cargo test -p lua-rs-runtime v54_v55_start_in_reported_generational_mode`
- CLI probes for `LUA_RS_VERSION=5.4` and `LUA_RS_VERSION=5.5`

### 1. Land #104: real GC byte accounting

Do this before touching generational policy. A generational collector depends on
real `gettotalbytes`/debt/estimate semantics; otherwise minor/major scheduling is
driven by fabricated observables.

Deliverables:

- Remove the Phase-B `totalbytes` refill/drop simulations in `api.rs`.
- Make `collectgarbage("count")`, `gcinfo()`, memory limits, GC debt, and heap
  pacing derive from real allocation/free accounting.
- Charge object payloads that are currently invisible to the pacer: table array
  storage, hash-node storage, string payloads, userdata payloads, closure/upvalue
  storage where applicable.
- Ensure every charged allocation has a single matching refund path on sweep or
  explicit release.
- Preserve uncollected/bootstrap object behavior so accounting cannot drift for
  objects that never enter the sweep lists.

Verification:

- Unit tests proving bytes return to baseline after allocate/grow/drop/full-GC.
- `cargo test -p lua-gc`
- official `gc.lua` and `gengc.lua` still pass without accounting simulations.
- GC canaries still pass in incremental and generational public modes.
- A stress test shows repeated allocation/collection plateaus instead of
  monotonically creeping.

### 2. Make incremental invariants real

Generational collection cannot be correct until the incremental collector can
maintain tri-color safety while the VM mutates live objects.

Deliverables:

- `keep_invariant()` reflects propagation/atomic phases.
- `is_sweep_phase()` reflects actual sweep states.
- Forward and backward barriers inspect object color/age and enqueue work rather
  than being inert.
- Barrier call sites for table writes, metatable writes, upvalue closure,
  userdata uservalue writes, proto/string/object installation, and API stores
  are audited against upstream `luaC_barrier*` sites.
- Gray-again/touched queues are represented explicitly enough for both
  incremental and generational modes.

Verification:

- Existing GC barrier canaries.
- New internal tests that create black parent to white/young child references
  through table, upvalue, metatable, userdata, and coroutine paths.
- official `tracegc.lua` where available, plus `gc.lua`.

### 3. Move finalizers into the collector

The current explicit pending-finalizer drain is not enough for faithful
generational sweeping.

Deliverables:

- Model finalizable-object lists equivalent to `finobj` and `tobefnz`.
- Preserve ordering and error behavior across Lua versions.
- Mark finalizable objects during atomic/separation phases rather than by an
  after-the-fact queue.
- Use and reset the reserved finalized state consistently.
- Handle finalizers in both normal and emergency/full collection paths.

Verification:

- Existing `__gc` multiversion tests.
- New tests for finalizer order, resurrection, errors, and OLD1/finalizer-list
  movement.
- official `gc.lua` finalizer sections.

### 4. Add generational object ages and cohorts

Only after the previous foundations are in place should the collector track
generational age.

Deliverables:

- Encode ages equivalent to `G_NEW`, `G_SURVIVAL`, `G_OLD0`, `G_OLD1`, `G_OLD`,
  `G_TOUCHED1`, and `G_TOUCHED2`.
- Maintain cohort boundaries for all collectable lists and finalizer lists.
- New allocations enter the correct age for the current collector state.
- Forward barriers can promote/age children as needed to preserve the
  generational invariant.
- Backward barriers move old touched objects into the appropriate gray/touched
  queues.

Verification:

- Internal age/color inspection tests before exposing any public claim.
- Tests matching the early `gengc.lua` table/metatable/upvalue age assertions.
- No regression in incremental mode.

### 5. Implement minor and major generational collection

Deliverables:

- `entergen`/mode transition does a full atomic pass and sweeps objects into old
  cohorts before declaring generational mode active.
- `youngcollection` marks roots, old-to-young edges, touched objects, finalizer
  lists, ephemerons, and weak tables correctly.
- `genstep` chooses minor vs major collection using real bytes, `GCestimate`,
  `lastatomic`, `genminormul`, and `genmajormul`.
- Bad minor collections fall back to major/full collection and then return to
  generational mode according to upstream rules.
- `collectgarbage("step", 0)` behavior matches the reference in both declared
  modes.

Verification:

- Deterministic minor/major scheduling tests with controlled allocation sizes.
- `collectgarbage("param", ...)` behavior for 5.5 remains correct.
- official `gc.lua`, `gengc.lua`, canaries, and `lua-gc` unit tests.

### 6. Finish weak table and ephemeron behavior

Deliverables:

- Weak values, weak keys, and ephemeron convergence participate in minor and
  major collection.
- Old weak tables touched by young entries are revisited in the right phase.
- Weak string/key accounting from #104 is reclaimed by the collector, not by
  public API cleanup shims.

Verification:

- Existing weak-table blocks in `gc.lua`.
- New generational weak-key/weak-value tests with old containers and young
  children.
- Repeated minor collections do not leak young objects reachable only through
  weak paths.

### 7. Build a real `testC` equivalent

#93 should not close while the strongest `gengc.lua` assertions are skipped.

Deliverables:

- Add an internal test helper, feature, or harness mode that exposes safe
  inspection equivalents for `T.gcage` and `T.gccolor`.
- Run the meaningful `gengc.lua` age/color assertions against lua-rs, or port
  the assertions into Rust tests with the same object graphs.
- Include table, metatable, userdata uservalue, upvalue, touched object,
  finalizer, weak-table, and mode-transition cases.

Verification:

- A failing test demonstrates the current fake/scaffold implementation is caught
  before the real generational implementation lands.
- The same test suite passes only once age/cohort/barrier behavior is real.

### 8. Final close gate for #93

#93 is done only when all of these are true:

- 5.4/5.5 hosted startup reports generational mode by default.
- Incremental mode still works and remains selectable.
- Generational mode performs real minor/major collection, not incremental
  collection plus a public-mode string.
- `gengc.lua` meaningful age/color assertions are exercised, not skipped.
- #104 simulations are gone.
- Collector-owned finalizers, write barriers, weak tables, and byte accounting
  are covered by tests.

Final verification command set:

- `cargo test -p lua-gc`
- `cargo test -p lua-rs-runtime --test multiversion_oracle`
- `./harness/canaries/gc/run_canaries.sh`
- `TEST_TIMEOUT_S=60 ./harness/run_official_test.sh reference/lua-c/testes/gc.lua`
- `TEST_TIMEOUT_S=60 ./harness/run_official_test.sh reference/lua-c/testes/gengc.lua`
- full official-suite sweep for 5.4 and 5.5 once the collector changes are in
  place.

## Issue/PR-ready summary

This branch fixes the narrow default-mode bug in #93: hosted Lua 5.4/5.5 now
start in reported generational mode, matching the reference startup path, while
raw state creation remains incremental. It does not implement real
generational GC. The remaining #93 work sits on top of #104 real byte
accounting and requires collector-owned finalizers, real barriers/invariants,
object ages/cohorts, minor/major scheduling, weak-table support, and a
`testC`-equivalent harness so `gengc.lua` age/color assertions are actually
exercised.
