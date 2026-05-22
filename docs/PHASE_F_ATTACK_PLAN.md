# Phase F: Attacking the remaining 11 official-test failures

State as of 2026-05-19 post-fix-bundle (`f5710c4`): **33/44 PASS (75%)** on the upstream Lua 5.4 test suite. This doc lays out how to drive the remaining 11 to passing, ordered by leverage and dependencies.

Run-time scoring is via `./harness/run_official_all.sh` → `harness/impl/official/run_all.tsv`.

## STATUS UPDATE 2026-05-19 LATE (post-H/G-2/G-3/audit/table refactor/R-α/small-fixes)

**Suite: still 33/44 PASS** but the failure floor moved significantly. Multiple agents ran in this session; net 0 flips, substantial structural progress.

**Major artifacts landed this session:**
- **Canonical `LuaTable` in `lua-types`** (`crates/lua-types/src/table.rs`, 1242 LOC, interior mutability via `RefCell<TableInner>`). Replaces the flat `Vec<(K,V)>` placeholder. `crates/lua-vm/src/table.rs` reduced to a 22-LOC re-export shim. `FLAT_TABLE_GROW_CAP` hack deleted; replaced with principled `TOTAL_GROW_CAP=1<<20` in lua-types.
- **`upvalue_get`/`upvalue_set` 3-tier resolution** (`state.rs:1978`): when an open upvalue's home thread is not the current thread, try borrowing the home thread's `LuaState` from `GlobalState::threads` directly (the gap that broke `gengc.lua:99`). Falls back to `cross_thread_upvals` mirror only when the home is borrow-locked or main thread.
- **Ghost abstraction audit infrastructure** (`docs/GHOST_ABSTRACTION_REGISTER.md`, `harness/check_ghost_abstractions.sh`, staged hook). 14 entries (8 floor + 6 surprise discoveries: interned-string strong roots, dual `Instruction`/`LexBuffer`/`LexState`/`ZIO`/`LuaDebug` types, scattered Phase-B todos). Bidirectional check catches both new ghosts and retired ones.
- **Canary set** (`harness/canaries/gc/`): 5 GC canaries + dual-mode runner. **Proved `gengc.lua` failure is NOT a generational GC bug** — `canary_b_coro_upvalue` reproduced the same failure under incremental mode. The whole gengc Phase-D-3 spend ($200) was avoided.
- **Many smaller bug fixes**: chunk_id @/=/empty source truncation correctness, runtime error `:line:` prefix routing (vm.rs arith ops + lex token NUL trim + parse syntax_error), `debug.getinfo.namewhat` → nil instead of "?", trace_exec C-frame guard, `reset_thread` actually wires `close_protected`/`set_error_obj` (was Phase-A no-op).

**Current failure profile (33/44):**

| Test | Failure point | Tier | Next action |
|---|---|---|---|
| `nextvar` | `:stdin:16 assertion failed` (deep in checkerror) | Medium | Investigate which checkerror call gets a different error message than expected |
| `coroutine` | `:stdin:327 assertion failed` | Hard | yield-through-pcall — R-β stalled chasing this; needs tighter scope |
| `locals` | `:stdin:982 assertion failed` | Hard | yield-inside-`__close` — R-γ stalled chasing this; needs tighter scope |
| `cstack` | "testing stack overflow detection" | Hard | GC marker queue stability under deep `coroutine.wrap` recursion |
| `db` | `:stdin:28 assertion failed` | Medium | line-hook tracker test — `debug.sethook("l")` event events don't match expected |
| `errors` | `:stdin:591 assertion failed` | Medium | Likely another error-message-prefix gap (deeper than the ones small-fixes hit) |
| `files` | `panicked at io_lib.rs:1563` | Easy-Medium | Another reachable `todo!()` we missed; same pattern as `is_closed` was |
| `gc` | TIMEOUT | Hard | Self-referenced threads section — GC walk doesn't converge; needs reachability cycle handling |
| `gengc` | `:stdin:130 assertion failed` | Hard (but NOT gen-GC) | Real GC barrier issue exposed by R-α — barriers are no-ops (see ghost register `gc-barrier-noops`) |
| `literals` | "cannot resume dead coroutine" | Medium | Different bug entirely — coroutine state after error |
| `all` | TIMEOUT | Composite | Downstream of `gc.lua` timing out; flips when `gc` does |

**Recommended next-session order:**

1. **`files:1563`** (~$10 sonnet) — another `todo!()` like H-2 found. Mechanical fix.
2. **`errors:591`** (~$15 opus) — another error-prefix gap; same pattern as small-fixes hit.
3. **`nextvar:16`** (~$20 opus) — checkerror got the wrong error message; trace which `pcall` is mis-erroring.
4. **`db:28` line-hook events** (~$30 opus) — debug.sethook("l") event firing semantics.

These 4 are ~$75 and could plausibly land **37/44**. The remaining 7 are all hard-tier slices ($30-50 each, $200-300 total):
- `coroutine` + `locals` + `cstack` are the same family (yield-through-C-frames, close-cascade, deep GC under coro recursion) — re-spec each individually with tighter scope than R-β/R-γ.
- `gc` + `gengc` are GC-correctness work; need write barriers (the `gc-barrier-noops` ghost) wired.
- `files` (after the :1563 fix) hits yield-across-C in `coroutine.wrap(io.lines)` — same yield-through-C family.
- `all` is downstream of `gc.lua`.

**Realistic target with full $300 next session: 39-41/44.**

---

(Prior status block below; retained for history.)

### STATUS UPDATE 2026-05-19 (post-F-1/F-2/F-3 + G-1)

**Suite**: still **33/44 PASS** but the composition changed and we hit the headline demo.

**Done:**
- F-1.a (path lookup), F-1.b (`tonumber`/`str_to_number` float fast-path), F-1.c (safe `_LOADED` walk in error formatting) — landed in `e465b33`.
- F-2.b (reachability-driven thread sweep) — `cstack`-as-free-win flipped briefly; re-regressed in F-3 integration. F-2.c (coroutine close cascade) — partial.
- F-3.a (`pcall_k` yieldable branch + `CIST_YPCALL`) and F-3.b (`call_k` yieldable branch for `dofile`/`pairs`/`ipairs`).
- F-3.c (parser linedefined / OP_RETURN0 fix bundled into F-3.b).
- **H-1 (heavy.lua regression fix)**: `FLAT_TABLE_GROW_CAP=8192` in `state.rs::LuaTableRefExt::raw_set` + `ARRAY_GROW_CAP=1<<20` in `table.rs::resize` emulate `LUA_ERRMEM` so for-loops over `math.huge` terminate. `heavy.lua`: TIMEOUT → PASS in 0.28s.
- **H-2 (io stubs + popen)**: `io_lib::is_closed` family filled via `lstream_from_upvalue` registry helper. `io.popen` end-to-end via new `PopenHook` + `PopenFile` impl in lua-cli.
- **G-1 (lfs-rs)**: full Rust-native LuaFileSystem (`crates/lua-rs-lfs/`, 8 fns, zero `unsafe`), wired into `package.preload` from lua-cli. `require("lfs")` returns a usable table.
- **Headline demo: LuaRocks 3.11.1 runs.** `luarocks --version` prints full help/version/config/rocks-tree output before failing on `error in error handling` at exit.

**Current failure set (11 tests):** see "Triage of the remaining 11" below.

## Prerequisite: v5 Stop-hook test gate

Before launching anything else, install the Stop-hook test gate from `harness/prompts/manual/05-stop-hook-test-gate.md`. This morning's bundle work caught 3 regressions that landed because the build-only gate is too loose. Every subsequent Phase F slice is at risk of the same pattern until v5 lands.

**Order: v5 gate FIRST, then everything below.**

## The 11 failures, classified

| Tier | Test | Current failure | Effort | Phase |
|---|---|---|---|---|
| **Easy** | literals | `attempt to concatenate a table value` | sonnet, $2-3 | F-1 |
| **Easy** | nextvar | `bad 'for' limit (number expected, got number)` (wording bug) | sonnet, $2 | F-1 |
| **Easy** | errors | `errors.lua:38 assertion failed` (specific checkmessage) | sonnet, $3 | F-1 |
| **Medium** | locals | `locals.lua:861 assertion failed` (deep, post-require) | opus, $10 | F-2 |
| **Medium** | gc | `gc.lua:552 assertion failed` (deep) | opus, $10 | F-2 |
| **Medium** | coroutine | `coroutine.lua:165 assertion failed` | opus, $10 | F-2 |
| **Hard arch** | files | `attempt to yield across a C-call boundary` (needs continuations) | opus, $30 | F-3 |
| **Hard arch** | cstack | `testing stack overflow detection` (C-stack limit) | opus, $20 | F-3 |
| **Hard arch** | db | `db.lua:50 assertion failed` (debug library) | opus, $30 | F-3 |
| **Defer** | gengc | `attempt to index a nil value (upvalue 'x')` (gen GC) | Phase D-3 | F-4 |
| **Defer** | all | `cannot open main.lua` (harness setup) | harness work | F-4 |

Total estimated spend to clear F-1 through F-3: **$135 in agent budget** + ~3 days human attention for design oversight.

## Triage of the remaining 11 (after F-1/F-2/F-3/H-1/H-2/G-1 integrated)

Failure points have shifted — most tests now fail deeper into the file. Re-classified by current root cause, not the original tier.

| Tier | Test | Current failure point | Root cause | Effort |
|---|---|---|---|---|
| **Trivial harness** | `all` | `cannot open main.lua` (composite test runner) | `all.lua` expects `dofile('main.lua')` to resolve from `testes/`; our `LUA_PATH` resolves but `dofile` is CWD-relative. Fix in `run_official_all.sh` (chdir before exec). | $0, 5 min |
| **Easy stdlib** | `errors` | `:388 attempt to index nil 'msg'` | `debug.getinfo(f).short_src` formatting near 60-char `[string "..."]` boundary. Likely a 1-line off-by-one in `auxlib.rs::chunkid`. | sonnet, $5 |
| **Medium coroutine** | `coroutine` | `:181 assertion failed` (`coroutine.close(co)` after `error`) | Close-after-error returns wrong msg (`100` expected, got something else). Path in `coro_lib::aux_close`. | opus, $20 |
| **Medium coroutine** | `locals` | `:982 to-be-closed in coroutines` | `__close` propagation through closed coroutines. Extends F-2.c. | opus, $30 |
| **Medium debug** | `db` | `:73 short_src formatting for empty chunkname` | `dostring(a, "")` produces `[string ""]`. Same family as `errors:388`. | opus, $20 |
| **Medium coro-close cascade** | `cstack` | "chain of coroutine.close" depth assertion | Deep close-chain should hit C stack overflow protection and report `"C stack overflow"`. Our nCcalls limit either too high or message wrong. | opus, $30 |
| **Medium io+coro** | `files` | `g_read should return at least one value` panic | `coroutine.wrap(io.lines)` path; H-2 advanced past `is_closed` but the new gap is `g_read` returning 0 values when called from inside a wrapped coroutine. | opus, $30 |
| **Medium GC interaction** | `literals` | silent abort | Mid-test segfault or unhandled panic; likely the same table-cap interaction as `nextvar` since literals.lua exercises large string-literal tables. | opus, $40 |
| **Hard structural** | `nextvar` | `not enough memory` (deliberate cap) | `for i=1,lim do a[i]=1 end` with `lim > 8192` trips the FLAT_TABLE_GROW_CAP. Real fix: wire `lua-vm/src/table.rs`'s rich array+hash impl into `LuaTableRefExt` instead of the flat-Vec placeholder. **Substantive refactor (Phase D table work).** | opus, $150 |
| **Hard structural** | `gc` | TIMEOUT in "self-referenced threads" | GC walk through self-referencing thread cycles; needs the reachability sweep to converge faster, or the thread cycle detection from upstream lgc.c. | opus, $80 |
| **Deferred** | `gengc` | `:99 attempt to index nil 'x'` | Cross-thread upvalue write barrier under generational GC. Phase D-3 work, intentionally deferred. | Phase D-3, $200+ |

**Verdict on "what's hard to fix":**
- **5 of 11 are tractable medium fixes** (`errors`, `coroutine`, `locals`, `db`, `cstack`) — ~$130 total, achievable in one chained run.
- **2 are easy wins** (`all` is just CWD fix; `errors` formatting is a 1-liner).
- **3 require structural work** (`nextvar`/`literals`/`gc` are all downstream of the table-impl swap or GC depth handling) — ~$270 combined, but `nextvar` unblocks two others.
- **1 is deferred by design** (`gengc` — Phase D-3).

A reasonable next push: clear the 7 tractable tests (~$150 in agent spend) to land **40/44** before tackling the table refactor.

---

## F-1: Easy wins (kick off in parallel after v5)

Three sonnet runs in parallel worktrees. Disjoint files. All should drop in <1 hour each.

### F-1.a literals.lua: `attempt to concatenate a table value`

**Likely cause**: A test in literals.lua tries `s .. t` where `t` is a table that has a `__concat` metamethod. Our concat path doesn't honor `__concat` correctly, or doesn't honor it at all.

**Investigation entry points**:
- `crates/lua-vm/src/vm.rs` — search for `OP_CONCAT` / `concat` opcode dispatch
- `crates/lua-vm/src/object.rs` — `luaO_str2num` and concat helpers
- C-Lua reference: `lvm.c::luaV_concat` (line ~775)

**Reproducer**:
```bash
PREAMBLE='_soft=true; _port=true; _nomsg=true; arg=arg or {}; _G=_G or _ENV'
src="$PREAMBLE"$'\n'"$(cat reference/lua-c/testes/literals.lua)"
target/debug/lua-rs "$src" 2>&1 | head -50
```
Find the test phase that triggers the error; bisect to the specific `..` expression.

**Acceptance**: literals.lua passes the harness scan.

### F-1.b nextvar.lua: error wording mismatch

**Cause**: `bad 'for' limit (number expected, got number)` is the error message we emit when a numeric-for loop's limit can't be converted. The wording is wrong — C-Lua says `'for' initial value must be a number` or similar variants depending on phase. Likely `for_error` in `crates/lua-vm/src/debug.rs`.

**Investigation**:
- `crates/lua-vm/src/debug.rs::for_error`
- `crates/lua-vm/src/vm.rs` — OP_FORPREP / OP_FORLOOP dispatch
- C-Lua reference: `lvm.c::forprep` and `ldebug.c::luaG_forerror` (line ~720)

**Reproducer**: read nextvar.lua:611 and the surrounding test; instrument as needed.

**Acceptance**: nextvar.lua passes.

### F-1.c errors.lua: checkmessage at line 38

Now that the `_G` recursion is gone, errors.lua hits a real assertion at line 38. That's inside `checkmessage(prog, msg)` (the same harness we documented for the morning errors.lua dispatch). The prompt template at `harness/prompts/errors.lua.txt` instructs the agent to:

1. Pre-instrument errors.lua to print the (prog, msg, actual) tuple for the failing case
2. Fix the specific wording mismatch in the VM
3. Don't re-add the `_G` walk (we just removed it)

**Acceptance**: errors.lua advances past line 38. Repeat the slice for each successive checkmessage failure (likely 5-10 cycles, ~$3 each).

---

## F-2: Medium (Opus, post-F-1)

Three opus runs. Each is a real diagnosis-then-fix job. Can run in parallel worktrees but cherry-pick sequentially.

### F-2.a locals.lua: assertion at line 861

The morning fix moved this from "tracegc not found" to a real assertion deep in the test. Line 861 is in the `<close>` attribute tests — Lua 5.4's to-be-closed variables.

**Investigation**:
- Read locals.lua 850-870 to identify the assertion
- Likely involves to-be-closed cleanup interacting with goto / break / return paths
- `crates/lua-parse/src/lib.rs` — TBC parsing
- `crates/lua-vm/src/func.rs::close` — TBC close machinery
- C-Lua reference: `lparser.c::checktoclose`, `lfunc.c::luaF_close`

### F-2.b gc.lua: assertion at line 552

Now reaches deep in the test post-budget-fix. Line 552 — probably weak-table edge case or finalizer count mismatch.

**Investigation**:
- Read gc.lua 540-560
- Likely one of: weak-key ephemeron iteration, finalizer ordering, `__gc` on userdata vs tables, or count-after-collect mismatch
- `crates/lua-types/src/trace_impls.rs` — weak-table trace
- `crates/lua-vm/src/state.rs::collect_via_heap` — post-mark hook
- C-Lua reference: `lgc.c::clearbyvalues` / `clearbykeys` / `GCTM`

### F-2.c coroutine.lua: assertion at line 165

Phase E got us to line 141 (coroutine.close stub) → bundle fix moved past close → now line 165. Probably an xmove edge case or status-machine mismatch.

**Investigation**:
- Read coroutine.lua 155-175
- Check whether `xmove` is correctly handling N-value transfers
- C-Lua reference: `lapi.c::lua_xmove`

---

## F-3: Hard architectural (Opus, post-F-2)

Each is a real slice, not a fix. Spec these out with their own `harness/prompts/manual/0X-*.md` files first.

### F-3.a files.lua: continuation support

Spec'd in this morning's TODO comment at `crates/lua-vm/src/api.rs:1772`. C-Lua's `lua_callk(L, nargs, nresults, ctx, k)` registers `k` as a continuation on the CallInfo; on resume after yield, `finishCcall` calls `k(L, status, ctx)` to continue the C code. Our port has stubbed `k = None` everywhere.

The slice:
1. Add `LuaKFunction` type alias for `fn(&mut LuaState, i32 /*status*/, isize /*ctx*/) -> Result<usize, LuaError>`
2. Wire `k` through `api::call_k` → `state.call_with_k(func, nresults, ctx, k)` → registers on CallInfo
3. `finishCcall` (do_.rs:1109) invokes the registered `k` on resume
4. `dofile_fn`, `pcall_k`, and other stdlib functions that need yield-across pass real continuations instead of `None`
5. Verify with files.lua's "yielding during dofile" test

**Estimated cost**: $30. Largest slice in F-3.

### F-3.b cstack.lua: C-stack overflow detection

C-Lua tracks `nCcalls` and aborts the Lua-side call when it crosses `LUAI_MAXCCALLS`. Our port has `nCcalls` and the constant but the check may not be wired in all the right places.

Investigation:
- C-Lua reference: `ldebug.c::stackerror`, `ldo.c::luaD_pretailcall`
- Search our codebase for `LUAI_MAXCCALLS`
- The test specifically validates that `f() f() f()...` deep recursion produces a clean Lua error, not a crash

### F-3.c db.lua: debug library completeness

`debug.getinfo`, `debug.getlocal`, `debug.setlocal`, `debug.gethook`, `debug.sethook` — these are partially wired but several edge cases fail.

Read db.lua:50 to find the first failing assertion. Each subsequent assertion is its own ~$5 fix. Probably 5-10 cycles to clear the whole file.

---

## F-4: Deferred (Phase D-3 / harness work)

### gengc.lua — generational GC

Requires age bits, old cohorts, back barriers, touched lists. Spec'd at high level in `docs/LUA_PHASE_E_RUNTIME_SPEC.md` Part 2. Real engineering — 1-2 weeks human + agent. Not on the immediate roadmap.

### all.lua — harness composition

all.lua does `dofile("strings.lua"); dofile("locals.lua"); ...` etc. Needs:
- `dofile` resolving relative to the directory of the running script (our `prepend_lua_path` helps for require but not for `dofile`)
- Test isolation between sub-test runs

Mostly mechanical once dofile-relative is in. ~$10.

---

## Dispatch order (after v5 gate lands)

**Round 1 (F-1, parallel)**: 3 sonnet worktrees on literals + nextvar + errors. ~1 hour, ~$10. Targets 36/44.

**Round 2 (F-2, parallel)**: 3 opus worktrees on locals + gc + coroutine. ~3 hours, ~$30. Targets 39/44.

**Round 3 (F-3.a, single)**: 1 opus on files.lua continuations. ~2 hours, ~$30. Targets 40/44.

**Round 4 (F-3.b + F-3.c, parallel)**: cstack + db. ~2-4 hours, ~$50. Targets 42-43/44.

**Defer**: gengc (Phase D-3) + all (harness).

**Total estimated**: $120-150 + ~10 hours human attention to dispatch and cherry-pick. End state: **42-43/44 PASS (95-98%)**.

---

## How to actually run this

After v5 lands:

```bash
# F-1 round
./harness/dispatch.sh manual/F-1a-literals.md &
./harness/dispatch.sh manual/F-1b-nextvar.md &
./harness/dispatch.sh manual/F-1c-errors.md &
wait
./harness/cherry_pick_worktrees.sh   # auto-cherry-pick all finished worktrees
./harness/run_official_all.sh        # measure

# F-2 round
# ...
```

(`dispatch.sh` and `cherry_pick_worktrees.sh` are TBD — wrapping the worktree-Agent + cherry-pick dance we've been doing manually. Worth building as part of the v5 work.)

## What "done" looks like

When the official-test pass count is 42+/44 AND the v5 gate is preventing per-commit regressions, the autonomous loop is effectively in production-quality territory. At that point the "Lua 5.4 in safe Rust runs LuaRocks" demo from PORT_STRATEGY.md §8 becomes the next milestone.
