# Embedding-API parity tier — attack plan & tracker reconciliation

**Date:** 2026-06-26
**Branch of record:** `feat/embedding-hard-tier` (shipped 0.3.7)
**Scope:** finish the omniLua host-embedding API to mlua-parity, and reconcile the
open GitHub issues against what this branch has *already silently implemented*.

This doc is the single source of truth for the push. The paste-able agent goal
references it; do not duplicate the tables into the goal.

---

## 0. Headline finding — the tracker is stale

The `feat/embedding-hard-tier` branch implemented several "open" issues as part of
the 0.3.7 hard-tier work but never closed them. Before doing *any* new work, the
done-but-open issues must be verified against the oracle and closed. The real
remaining surface is ~6 issues, not 10.

Grounding evidence was gathered by grepping `crates/lua-rs-runtime/src/lib.rs`,
`crates/lua-vm/src/state.rs`, `crates/lua-stdlib/src/coro_lib.rs`, and the
integration tests in `crates/lua-rs-runtime/tests/`.

---

## 1. Reconciliation table — claimed vs. actual

| Issue | Pri | Claimed | Actual on branch | Evidence |
|---|---|---|---|---|
| **#227** chunks `Chunk::into_function` | high | open | ✅ **DONE + tested** | `lib.rs:1842`; `tests/compiled_chunk.rs` |
| **#229** tracebacks to host | med | open | ✅ **DONE + tested** | `Error::traceback_bytes/_lossy`, `set_capture_tracebacks`; `tests/traceback_capture.rs` |
| **#235** cross-instance bridge `marshal_from` | low | open | ✅ **looks DONE + tested** (cycle-safe recursion + `seen` set) | `lib.rs:4091`; `tests/cross_version_bridge.rs` |
| **#232** table ergonomics | low | open | 🟡 **HALF** — `push/insert/remove/clear` done+tested; **lazy `__pairs` iterator NOT done** (still `raw_pairs()→Vec` at `lib.rs:2081`) | `lib.rs:3826`; `tests/table_helpers.rs` |
| **#226** registry | med | open | 🟡 **HALF** — `set_/named_/unset_named_registry_value` done+tested; **keyed `RegistryKey` API NOT done** (no `RegistryKey` type exists) | `lib.rs:3758+`; `tests/named_registry.rs` |
| **#234** WebLua Engine/Backend seam | high | open | 🟡 **SLICE 1 only** — number-model marshaling (`LossyIntPolicy`, `lower_host_int`) done+tested; **`enum Engine` / `Backend` trait / `Unsupported` registry NOT done** | `lib.rs:927+,1957`; `tests/number_seam.rs` |
| **#230** host-driven coroutines | med | open | ❌ **NOT started** — `Thread` has only `to_pointer`; no `create_thread`/`resume`/`status` | `lib.rs:3690` (impl Thread) |
| **#231** GC control surface | low | open | ❌ **NOT started** — only `Lua::gc_collect()` | `lib.rs:1757` |
| **#239** `resume(running())` wording bug | bug/5.4 | open | ❌ **NOT fixed** | `state.rs:~1808`, `coro_lib.rs:~229/283` |
| **#113** GC pacing / object diet (RSS) | med/arch | open | ❌ **NOT fixed** | `state.rs` `generational_step`/`stepgenfull` |

---

## 2. The plan — three tracks

### Track 1 — Reconcile (do first; cheap, read-only + `gh issue close`)
Verify the done-but-open set passes acceptance, then close with an evidence comment.
- **#227** → `cargo test -p lua-rs-runtime --test compiled_chunk`
- **#229** → `cargo test -p lua-rs-runtime --test traceback_capture`
- **#235** → `cargo test -p lua-rs-runtime --test cross_version_bridge`
Each must also survive the multiversion oracle. If green, close. If a gap shows,
leave open and record the gap here.

### Track 2 — Finish the parity tier (the headline batch → next minor release)
Cohesive: all in `lua-rs-runtime` + `lua-vm`/`lua-stdlib`, each with an oracle
acceptance. Ordered by dependency, smallest-cause-first:

1. **#239** (small; the domino — do before #230). Main thread is not registered in
   `GlobalState.threads` (`state.rs:~1808`), so `aux_resume` treats it as *dead*
   while `aux_status` treats it as *normal* — `resume(running())` says
   `cannot resume dead coroutine` instead of `...non-suspended...`. Fix: register
   the main thread, or distinguish the not-found path in `aux_resume`
   (`coro_lib.rs:~283`) from a genuinely dead coroutine. Capture exact per-version
   wording via `specs/oracle/diff_one.sh` and add the case to `multiversion_oracle`.
2. **#230** (headline; gated by #239). `Lua::create_thread(Function)`,
   `Thread::resume::<A,R>(args)`, `Thread::status()->ThreadStatus`,
   provenance-checked to the parent like other handles. New `tests/host_coroutine.rs`;
   behavior must equal running the same coroutine purely in Lua (Suspended→Dead).
3. **#226** (finish the half). Keyed `RegistryKey` API
   (`create_registry_value`/`registry_value::<T>`/`remove_registry_value`) layered
   on the existing rooting machinery (`RootedValue`/`ExternalRootKey`), provenance-
   checked to its parent `Lua`. Extend `tests/named_registry.rs`.
4. **#231** (small). `Lua::gc()` handle: `collect`/`step(kb)`/`stop`/`restart`/
   `count()->f64`/`is_running()`; version-divergent knobs return
   `LuaError::Unsupported`. New test: `count()` rises after alloc, falls after
   `collect()`; `stop()`/`restart()` gate auto-collection.
5. **#232** (finish the half). Lazy `pairs()`/`raw_pairs()` iterator that does not
   materialize the `Vec` up front; honor `__pairs` on 5.2+. Extend `tests/table_helpers.rs`.

### Track 3 — Defer to deep-spec → codex-review → execute (NOT in the batch)
Per the standing preference for correctness-sensitive architectural work, these
each want their own spec + cross-model adversarial review before code:
- **#234-full** — `enum Engine` (closed, `#[cfg]`-gated) + `Backend` seam-contract
  trait + machine-readable `Unsupported` divergence registry. The real
  multi-version differentiator (`specs/WEBLUA_MULTIVERSION_API_SPEC.md` §4.1/§3.4/§6).
- **#113** — generational GC pacing convergence / object diet. Perf; bisect-grade
  care (the `lastatomic`/`stepgenfull` non-convergence). Separate perf session,
  measured per `docs/MEASUREMENT_PROTOCOL.md`.

---

## 3. Parallelization analysis

**The constraint:** Track-2 items #226/#230/#231/#232 all land in the same hot file,
`crates/lua-rs-runtime/src/lib.rs`. Per `CLAUDE.md`, never run two file-editing
agents in one worktree, and the additive method blocks would conflict if edited in
parallel in a shared tree. So **most of Track 2 is serial by physics, not choice.**

**The one clean parallel split** (each in its own `git worktree`):

| Lane | Issues | Files touched | Why isolatable |
|---|---|---|---|
| **Lane R (read-only)** | Track 1 reconcile (#227/#229/#235) | none (tests + `gh issue close`) | No edits — safe anywhere, even main worktree, concurrent with everything |
| **Lane A (vm/stdlib)** | #239 | `lua-vm/state.rs`, `lua-stdlib/coro_lib.rs`, `multiversion_oracle` | Disjoint from `lib.rs`; no overlap with Lane B |
| **Lane B (runtime leaves)** | #226 + #231 + #232 | `lua-rs-runtime/lib.rs` (+ its tests) — **serial within the lane** | Independent *logic*, shared *file* → one worktree, one agent, sequential |

**Sequencing rule:**
- Lanes R, A, B can run **concurrently** (3 worktrees).
- **#230 is the integration point** — it edits `lib.rs` *and* depends on #239's
  correctness. Do it **last**, after Lane A (#239) is merged and Lane B has landed
  its `lib.rs` changes, to avoid a three-way `lib.rs` churn. Either land it in
  Lane B's worktree after B finishes, or rebase it onto the merged result.

**Recommendation if not parallelizing:** just do it serially in this worktree in
the Track-2 order (1→5). The parallel speedup here is modest (one extra worktree
for the vm/stdlib bug) and `lib.rs` forces a serial spine regardless. Parallelize
only if you want the reconcile + the coroutine bug off the critical path.

---

## 4. Done gate (before declaring the batch shippable)
- `cargo test --workspace` green
- `harness/run_official_all.sh` green
- `specs/oracle/check.sh` (×5 per the PR-gate rung)
- hooks satisfied: no-inline-comment, PORT STATUS trailer, unsafe-budget, forbidden-import
- Summarize what closes and what ships as the next minor; #234-full and #113 remain open with a pointer to Track 3.
