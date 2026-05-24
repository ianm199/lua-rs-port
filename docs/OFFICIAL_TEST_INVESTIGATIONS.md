# Official-test failure investigations

Living document tracking the deep-dive into each failing test in `harness/run_official_all.sh`'s 44-test suite. Last honest measurement (2026-05-24): **37/44 (84%)**.

Each section records: what the test exercises, the precise failure, upstream (lua-c) data flow for the same source, our current behavior, the divergence, attempted fixes, and what's still required.

---

## Tracking convention

| status | meaning |
|---|---|
| **OPEN** | actively investigating |
| **DIAGNOSED** | root cause understood; fix not yet attempted |
| **PARTIAL** | some fixes landed but test still fails |
| **FIXED** | test passes in HEAD |
| **PARKED** | known-cause but out of scope (e.g. needs a different subsystem) |

---

## `db.lua` — line-hook attribution

**Status**: PARTIAL — db.lua internally progresses (simple test() cases pass) but the gap-test loop at line 207 still fails.

### What it tests

`debug.sethook(f, "l")` fires `f` on every line executed. The test compares the fired-line sequence against a hand-written expected list. Each test() invocation runs a small Lua chunk and asserts the trace matches.

### Failure point

`db.lua:28` — `assert(l == line, "wrong trace!!")` inside the test() helper.

The helper is called many times; the failure is whichever test() invocation hits a trace mismatch first. After our surgical fixes, only the **gap-test loop at line 207** still fails. That loop generates 256 test() invocations with `\n×i / \n×j` injections inside `a = b[1] + b[1]` to stress line attribution across large source gaps.

### Root cause (the systemic finding)

**lua-c attributes every instruction's line via `savelineinfo(fs, f, fs->ls->lastline)` at emit time**, called from `luaK_code` (lcode.c:388). The line is read DYNAMICALLY at the moment the instruction is added to the proto.

**Our parser threads a `line` parameter through 30+ codegen call sites**, captured at various points before the actual emission. For simple statements this matches lua-c. For nested expressions where emit happens many tokens after capture, the saved `line` is stale.

### lua-c data flow for `a = b[1] + b[1]`

`luac -l -l` dump:
```
1 [1] VARARGPREP   0       ; line 1
2 [1] NEWTABLE     0 0 1   ; line 1
3 [1] EXTRAARG     0       ; line 1
4 [1] LOADI        1 10    ; line 1
5 [1] SETLIST      0 1 0   ; line 1
6 [3] GETI         1 0 1   ; line 3 — first b[1]
7 [4] GETI         2 0 1   ; line 4 — second b[1]
8 [3] ADD          1 1 2   ; line 3 — operator
9 [3] MMBIN        1 2 6   ; line 3
10 [4] SETTABUP    0 0 1   ; line 4 — store back to a
11 [5] LOADI       0 4     ; line 5 — b = 4
12 [5] RETURN      1 1 1   ; line 5
```

Key dynamic behavior:
- The first GETI is emitted AFTER `+` is consumed → `lastline = 3` → instruction at line 3
- The second GETI is emitted AFTER second `b[1]` is parsed → `lastline = 4` → instruction at line 4
- ADD/MMBIN emit at current `lastline = 4`, but lua-c calls `luaK_fixline(fs, line)` after emission to OVERRIDE the line to the saved operator line (3)
- SETTABUP emits at current `lastline = 4`

### Our data flow

We capture `let line = ls.linenumber;` BEFORE `lex_next` consumes the operator → `line = 3` (operator line). This is correct for the operator-line capture but is then passed all the way down to `cg_discharge_vars` for both operands.

Result: our second GETI also emits at line 3, missing the line-4 hook fire.

### Surgical fixes that landed

| commit | site | from | to |
|---|---|---|---|
| `deacc5e` | `adjust_assign` (lib.rs:2443) | `linenumber` | `lastline` |
| `deacc5e` | `leave_block` OP_CLOSE (lib.rs:2898) | `linenumber` | `lastline` |
| `deacc5e` | `whilestat` back-jump (lib.rs:3895) | `linenumber` | `lastline` |

These three flipped these patterns to pass:
- `local a` (with no initializer) now fires hook for line 1
- While-loop back-jump no longer fires the END line on every iteration
- `do...end` blocks attribute their OP_CLOSE to the END's line, not the next-statement's line

All simple test() cases (lines 124–188 in db.lua) pass standalone.

### What's still required

The gap-test loop needs lua-c's dynamic-lastline-at-emit behavior. Either:

1. **Structural emit_inst refactor**: change `emit_inst(fs, line, inst)` to ignore the threaded `line` and read `fs.last_token_line` (mirrored from `ls.lastline` on every `sync_from_lex`). Plus add a `luaK_fixline`-equivalent to override the line of just-emitted instructions for binop tag-method line attribution.
2. **Per-site audit**: walk every `cg_discharge_vars` / `cg_exp_to_*` callsite and change it to use the *current* `ls.lastline` instead of the threaded `line`. ~30 sites.

### Attempted but reverted

- **Bulk `linenumber → lastline` replace (2026-05-24)**: regressed `errors.lua` from PASS→FAIL on `lineerror` cases that depend on operator-line capture for binop tag-method attribution. Specifically the binop site in `subexpr` (lib.rs:3645) needs `linenumber` (operator's line, captured BEFORE consuming) — bulk replace got the line of the LEFT OPERAND instead.
- **Add `fs.last_token_line` field + read from `emit_inst`**: introduced a deep crash (`internal: execute called on non-Lua frame`) in db.lua. Not yet diagnosed; probably caused a wrong abslineinfo offset that mis-set CallInfo state somewhere.

---

## `gc.lua:469` — weak-table-of-long-strings sweep

**Status**: OPEN — collectgarbage("count") returns honest values; need to find what gc.lua's preconditions look like at the failing assertion.

### What it tests

Lua 5.4's incremental GC handling of weak tables (`__mode = "kv"`) holding long-string keys mapped to mixed values (numbers, tables). After collect, only entries with non-collectable values should survive.

### Failure point

`gc.lua:469` — `assert(collectgarbage("count") >= m + 2^12 and collectgarbage("count") < m + 2^13)` — expects post-collect count to be between 4 MB and 8 MB above baseline.

### Standalone repro behaves correctly

```lua
local m = collectgarbage("count")              -- ≈ 0.3 KB
local a = setmetatable({}, {__mode = "kv"})
a[string.rep("a", 2^22)] = 25                  -- long string -> number
a[string.rep("b", 2^22)] = {}                  -- long string -> table
a[{}] = 14                                      -- table -> number
-- check post-insert count > m + 8192 ✓ (8192.8 KB)
collectgarbage()
-- check post-collect 4096 ≤ count < 8192 ✓ (4096.7 KB)
```

Our output:
```
baseline m = 0.3017578125 KB
after insert: diff = 8192.81 KB (passes assert > 8192)
after collect: diff = 4096.68 KB (passes assert 4096 ≤ ... < 8192)
surviving entries in a: { key=4194304-char string, val=25 }
```

**Standalone behavior is correct**. So gc.lua's failure must come from accumulated state from earlier tests in the file — `m = collectgarbage("count")` at the test's start might be a value where `+2^13` overflows our actual heap delta.

### Next investigation step (UPDATED)

**Surprise: gc.lua's actual failure mode is TIMEOUT, not the assertion.** Running gc.lua with even a 5-minute budget doesn't reach line 469. The instrumented probe printed `long strings → steps → steps (2)` (gc.lua:183) and then never advanced. So our `gc` failures in the suite are happening during gc.lua's GC-torture-test loops (`steps (2)` at line 183+), and the "line 469 assertion failed" we see in some runs is from variance when the loops happen to complete fast enough.

The underlying issue is GC performance on the "steps (2)" test: gc.lua executes ~1000 `collectgarbage("step", N)` calls and expects each to do bounded work. Our incremental-step implementation is doing far more work per step than reference's, blowing the time budget.

**Next**: profile gc.lua up to line 200 (the steps loop), find what's slow. May be the unpause-the-heap fix from this session making auto-collect fire too aggressively. Park for now — needs perf investigation, not a correctness fix.

---

## `gengc.lua:122` — generational GC

**Status**: PARKED — needs real generational-GC implementation, out of scope for this session.

The test calls `collectgarbage("step", 0)` after entering generational mode and asserts `T.gcage(t) == "old"` etc. Our `change_mode(Generational)` is essentially a no-op; we don't have a separate young/old chain or barrier-back mechanism. ~500–1000 LOC of GC work.

---

## `coroutine.lua:319` — xpcall + yield + error

**Status**: FIXED for this specific test (coroutine.lua now fails at line 352, a different per-coroutine-hook test).

### Root cause

lua-c's `luaG_errormsg` invokes the message handler at the error-RAISE site (inside the longjmp path), so by the time `finishpcallk` sees the error during recovery, the message has already been transformed to `handler(err)`.

Our Rust error propagation uses `Result::Err` and has no equivalent chokepoint at raise. The synchronous-error path in `pcall_k` (api.rs:1944) handles the handler call inline. But the yield-then-error path (`pcall_k` Err(Yield) at api.rs:1930) propagates Yield without restoring state and the eventual resume-time error skips the handler entirely — `msg` reaches the test as the raw error table instead of `g(err)`.

### Fix landed

`crates/lua-vm/src/do_.rs` `finish_pcallk` — after `set_error_obj` writes the error to `func_idx`, invoke the handler (via `state.errfunc`, which is preserved across yield since the Yield path never restored it) and replace the error value. Mirrors lua-c's `luaG_errormsg` semantically — different timing (recover instead of raise), equivalent post-condition. The synchronous-error path clears `CIST_YPCALL` before returning so it never reaches `finish_pcallk` — no double-invoke.

### Verification

Standalone repro `/tmp/coro_repro.lua` returns `r=false, msg=240` (was `msg=table`). errors.lua's xpcall-based `checkerror` cases unaffected (verified via Tier 2 regression check).

### Next failure surface in coroutine.lua

Line 352: `assert(#trace == #correcttrace)` — the per-coroutine hook test (`debug.sethook(co, fn, "clr")`). Our `debug_lib::set_hook` has a TODO for the `!target_is_self` path — we don't install hooks on a different coroutine's state. Naive fix (borrow_mut the target's `Rc<RefCell<LuaState>>` and install) was attempted but introduces borrow conflicts when the target is later resumed and its own `coroutine.status` check tries to borrow. Proper fix likely needs the hook closure stored in `GlobalState` keyed by `thread_id`, looked up at hook-fire time. ~100–200 LOC of careful refactoring.

---

## `locals.lua:974` — `<close>` across coroutine exit paths

**Status**: DIAGNOSED — close-order is correct without yields; `coroutine.yield` from inside `__close` is the failure mode. Park for deep fix.

### What it tests

Lua 5.4's `local x <close> = ...` semantics: when scope exits (normally OR via error), each `<close>` variable's `__close` metamethod fires in LIFO order. The test stresses the case where each `__close` itself contains `coroutine.yield(...)`.

```lua
local function foo (err)
  local z <close> = func2close(function(_, msg)
    assert(...)
    coroutine.yield("z")     -- yield from inside __close
  end)
  local y <close> = func2close(function(_, msg)
    coroutine.yield("y")
    if err then error(err + 20) end
  end)
  local x <close> = func2close(function(_, msg)
    coroutine.yield("x")
  end)
  if err == 10 then error(err) else return 10, 20 end
end
```

### What works

Without yields, our `<close>` ordering is correct:

```
foo raising error(10)
x close: msg=10, err=10   <- x fires first (LIFO ✓)
y close: msg=10, err=10   <- y fires second; raises error(30)
z close: msg=30, err=10   <- z fires last with the updated error
final: false, 30
```

### What breaks

With yields inside __close (the actual test), our impl reports z firing FIRST (wrong order) on the third foo case (`pcall(foo, 10)`). The first `co()` call after entering this scenario returns `false, "...assertion failed!"` where the failing assert is z's `assert(msg == nil or msg == err + 20)` — z saw `msg=10` (the original error), not `msg=30` (the y-substituted error). This means z fired before y had a chance to run and substitute.

### Root cause (hypothesis)

When `__close` yields, our impl pauses x's close mid-execution. On resume, control should resume INSIDE x's close (just past the yield), let it return, then proceed to y. Instead it seems the close-list is re-walked from the top after resume, hitting z (or whatever is currently at the top of `tbclist`). The yield interrupts the close-list walk and our recovery path doesn't preserve "which entry is mid-flight."

### Fix shape

The `close` function in `crates/lua-vm/src/func.rs:530` is a simple `while state.tbclist.last() >= level { pop; prep_call_close_mth(...) }` loop. If `prep_call_close_mth` yields, the iteration state is lost. lua-c handles this via `CIST_CLSRET` CallInfo flag — the resumed __close call has CIST_CLSRET set, signalling to poscall that this is a close-method return and the list-walk should continue from where it left off (not be restarted).

Look for our handling of CIST_CLSRET (we have the constant; we may not handle it correctly in the yield-resume path). Likely fix: store the "currently being closed" tbc index on the CallInfo, restore on resume, continue from there instead of re-popping.

Estimated effort: 100-200 LOC, plus a chunk of yield-from-C state-machine work. Higher risk than coroutine.lua's fix because the bug spans VM + close-list + yield interaction.

---

---

## `cstack` — Rust stack overflow during 1000-coroutine close cascade

**Status**: PARTIAL — chain cascade bound via `co_close` `inc_c_stack` works standalone; cstack.lua still SIGSEGVs due to a tracegc `__gc` interaction with the cascade.

`coro_lib.rs:486` `co_close` now calls `inc_c_stack` so cascading closes hit `LUAI_MAXCCALLS` (200) and return `"C stack overflow"` cleanly. Verified standalone with a 1000-coroutine chain. But `cstack.lua` enables `tracegc` (a `__gc` finalizer that writes `.` to stderr and re-marks the object for next-cycle finalization) which somehow interacts badly with the close cascade — SIGSEGV before the bound trips.

Hypothesis: an automatic GC fires during the cascade (now that the heap auto-collector is unpaused), and the `__gc` invocation enters Rust code that recurses through the active coroutine chain.

---

## `all` — meta-runner

**Status**: AUTO — wraps every other test, fails because subtests fail. Will pass automatically when the other 6 pass.

---

## The structural emit_inst design (what would actually fix this layer)

For a future session, the right systemic fix:

1. Add `fs.last_token_line: i32` field, mirrored from `ls.lastline` in `sync_from_lex` and in each `lex_next` / `lex_lookahead` call site (currently `sync_from_lex` already runs after both).

2. Change `emit_inst` to use `fs.last_token_line` for the line attribution, completely ignoring the threaded `line` parameter (or using it only as an override hint).

3. Add a `cg_fixline(fs, line)` helper that overrides the line of the just-emitted instruction (the latest entry in `f.lineinfo` / `f.abslineinfo`). Mirrors lua-c's `luaK_fixline`.

4. In `cg_posfix_fold`'s binop emission and a few other sites where lua-c explicitly fixlines, call `cg_fixline` after the emission.

When attempted naively (just changing emit_inst), this introduced a deep crash because the `abslineinfo` records were mis-aligned vs the threaded line — needs careful audit of `previousline` updates and `iwthabs` accounting against the new effective_line.

Estimated effort: 4–6 hours of careful work + extensive testing.

---

## Honest progress today

Started the session at **38/44** (variance-inflated). Ended at **37/44** (deterministic). Net pass-count unchanged but:

- `gc_count` / `gc_count_b` now report real `heap.bytes_used` plus tracked long-string bytes
- `co_close` now bounds Rust stack via `inc_c_stack` (cascade works standalone)
- Three lua-c-faithful line-attribution fixes (`adjust_assign`, `leave_block` OP_CLOSE, while back-jump)
- Crystallized understanding of the lua-c vs lua-rs line-attribution divergence (this document)

The gap-test fix for db.lua is a known structural change away. The remaining failures are now well-characterized.
