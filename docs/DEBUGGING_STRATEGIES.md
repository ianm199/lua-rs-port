# Debugging Strategies

Reusable debugging patterns that worked during the Lua port. These are meant to
be operational playbooks, not postmortems.

## Honest Repro Ladder

Use this when a failure is subtle, long-running, or has already burned multiple
sessions. The central idea is to shrink the loop without changing the semantic
shape of the failing test.

### Steps

1. **Start from the real failing sequence**

   Copy the official failing block as literally as possible. Preserve the same
   number of resumes, calls, protected-call boundaries, and final assertions.
   A smaller repro is only useful if it keeps the same state-machine shape.

2. **Make the toy prove its own assumptions**

   Print or assert every intermediate yield/result, not just the final failure.
   If the toy expects the second scenario's result while it is still draining the
   first scenario, the toy is lying.

3. **Classify the first divergence, not the most visible one**

   Once the exact mini-test exists, find the earliest step where behavior differs.
   That usually points to the subsystem boundary. In the `locals.lua` case:

   - no-error close/yield passed;
   - close-error substitution passed;
   - body-error-before-close failed.

   That ruled out the close-list iterator and moved attention to yieldable
   `pcall` recovery.

4. **Use the source implementation as a state-machine oracle**

   Compare the exact control path in Lua C, not just the function names. Look for
   who owns the protected boundary, which flags stay set across unwind, and which
   stack slots act as continuation state.

5. **Patch the boundary, then let the suite reveal the next edge**

   Do not overfit the toy. After the first fix, rerun the official target. If it
   advances to the next adjacent case, add that case to the toy only if it covers
   a new state-machine edge.

6. **Record the wrong hypothesis**

   Saving the discarded theory matters. Here, "we need a close-list cursor" was
   plausible but wrong. Lua C uses the popped `tbclist` entries as the cursor;
   the real bug was losing `CIST_YPCALL` before `precover`.

### Case Study: `locals.lua` close/yield

Initial belief: yielding inside `__close` lost close-list iteration state.

What changed the diagnosis:

- The first repro was too short and under-drained the coroutine.
- Replacing it with the exact 12-resume official sequence showed the close-error
  path already worked.
- The remaining failure happened when a body error entered a yieldable `pcall`
  before any `__close` method yielded.

Actual fix:

- Match `lua_pcallk` in `lapi.c`: yieldable `pcall_k` should call directly and
  let `lua_resume` be the protected boundary.
- Preserve `CIST_YPCALL` across real errors so `precover -> finishpcallk` runs
  yieldable close recovery.
- Close errored wrapped coroutines in `coroutine.wrap`, matching `luaB_auxwrap`.

Fast-loop artifact:

- `harness/repro/locals_close_yield_repro.lua`

### When To Use This

- Coroutine/resume/yield bugs.
- Protected-call or error-recovery bugs.
- VM op replay bugs.
- GC or finalizer bugs with multiple phases.
- Any test where a "minimal" repro may accidentally remove the failing state.

### Failure Smell

If the toy repro fails differently from the official test, do not patch against
it yet. First make the toy explain every transition until the first divergence
matches the official failure.

## Official-Test Crash/Hang Ladder

Use this when an official test fails only after hundreds of lines of setup, or
when the visible failure is allocator corruption, a timeout, or an assertion
with no useful source line. The goal is to turn the problem into a sequence of
short, falsifiable loops.

### Working Rules

1. **Preserve lifecycle shape before minimizing size**

   A toy repro must keep the important state transitions: hooks, explicit or
   implicit GC, protected calls, coroutine resume/yield, open upvalues,
   finalizers, traceback/string allocation, and stack unwinding. A very small
   repro that removes one of those transitions can pass while the official test
   still fails.

2. **If the first toy passes, add back whole phases**

   Do not immediately suspect nondeterminism. Add back the nearest official
   phase that changes ownership or stack shape. For `db.lua`, the hook plus
   coroutine traceback toy passed; the useful repro had to include the
   `debug.getlocal` / `debug.setlocal` / open-upvalue phase before the coroutine
   traceback loop.

3. **Treat crash location as the first detection site**

   Allocator crashes often show the victim allocation, not the bad write. In
   `db.lua`, malloc aborted while allocating a traceback line in
   `string.gmatch`, but ASan showed the real bug was an `UpVal` object swept
   during an earlier `coroutine.resume`.

4. **Once the repro is short, switch to ASan**

   `lldb` is good for confirming the visible crash stack. ASan is better for
   use-after-free: it gives the allocation stack, free stack, and later bad
   access stack.

5. **After every crash fix, rerun the official test**

   Memory fixes often reveal the next semantic failure. Do not assume the
   original official test is done because the crash repro passes. In `db.lua`,
   fixing the UAF exposed a swallowed finalizer assertion, then a traceback
   truncation loop, then stripped-debug-info mismatches.

6. **Use prefix checkpoints to find the next live section**

   Once a crash becomes a hang/assertion, run `1..N` prefixes around visible
   section boundaries. This is much faster than inspecting the entire file.

7. **For GC, root both the value and the handle object**

   A stack snapshot may keep the pointed-to `LuaValue`s alive while still
   allowing the GC object that represents an open upvalue, closure, thread, or
   table to be swept. If a later root list still contains that handle, the next
   mark phase will touch freed memory.

8. **Hangs can be swallowed assertions**

   A repeat-until loop can look like "GC never ran" when the finalizer actually
   ran, failed inside a protected call, and left the loop condition unchanged.
   Instrument the body before assuming scheduler or GC starvation.

### Fast Commands

Run one official target:

```sh
RUSTFLAGS='-Awarnings' ./harness/run_official_test.sh reference/lua-c/testes/db.lua
```

Run a focused repro in a loop:

```sh
for i in {1..50}; do
  target/debug/lua-rs harness/repro/db_hook_getlocal_coroutine_traceback_repro.lua >/tmp/repro.out 2>/tmp/repro.err || {
    echo FAIL:$i:$?
    cat /tmp/repro.out
    cat /tmp/repro.err
    exit 1
  }
done
echo ok
```

Use `lldb` for the visible abort stack:

```sh
lldb --batch -o run -k 'bt all' -- target/debug/lua-rs harness/repro/db_hook_getlocal_coroutine_traceback_repro.lua
```

Build and run ASan after the repro is short:

```sh
RUSTC_BOOTSTRAP=1 \
RUSTFLAGS='-Zsanitizer=address -Cforce-frame-pointers=yes' \
CARGO_TARGET_DIR=target/asan \
cargo build -p lua-cli

ASAN_OPTIONS='detect_leaks=0:halt_on_error=1:abort_on_error=1:symbolize=1' \
target/asan/debug/lua-rs harness/repro/db_hook_getlocal_coroutine_traceback_repro.lua
```

Run an official-file prefix:

```sh
src=$(sed -n '1,916p' reference/lua-c/testes/db.lua)
target/debug/lua-rs -e "_soft=true; _port=true; _nomsg=true; _U=false; arg=arg or {}; _G=_G or _ENV; if _VERSION==nil then _VERSION='Lua 5.4' end; assert(load([====[$src]====], '@reference/lua-c/testes/db.lua'))()"
```

### Case Study: `db.lua`

Visible failure:

- Full `db.lua` aborted in malloc while `string.gmatch` pushed a traceback
  capture.
- The allocation was a traceback line string, but this was only where malloc
  detected earlier heap damage.

What shortened the loop:

- A tiny hook-plus-coroutine traceback repro passed, so it had removed a needed
  phase.
- The useful toy preserved the earlier debug-local/open-upvalue section:
  `harness/repro/db_hook_getlocal_coroutine_traceback_repro.lua`.
- That repro aborted immediately and passed after the real UAF fix.

How the UAF was found:

- `lldb` confirmed the visible crash was still `LuaString::from_bytes` called
  from `string.gmatch`.
- ASan reported a write to a freed `UpVal` header during `Marker::mark`.
- Allocation stack: `find_upval -> new_upval_open`.
- Free stack: GC during `coroutine.resume`.
- Later bad access: `LuaState::trace` iterating the parent thread's
  `openupval` list.

Actual UAF fix:

- The coroutine resume path already snapshotted suspended parent stack values
  in `GlobalState::suspended_parent_stacks`.
- It now also snapshots suspended parent open-upvalue handles in
  `GlobalState::suspended_parent_open_upvals`.
- `GlobalState::trace` traces both snapshots.
- Lesson: keeping the parent stack values alive is not enough; the `UpVal`
  objects in the parent `openupval` list are GC objects too.

Next failure: finalizer loop timeout:

- Prefix slicing showed the hang entered `db.lua:903..916`.
- The finalizer did run, but `debug.getinfo(1)` inside `__gc` returned no
  metamethod name, so the finalizer assertion failed under protected call and
  left `name == nil`.
- Fix: mark the caller frame with `CIST_FIN` while invoking the finalizer so
  `funcnamefromcall` reports `namewhat="metamethod"` and `name="__gc"`.
- Lesson: a hang can be a swallowed assertion inside a protected finalizer.

Next failure: traceback-size timeout:

- The block at `db.lua:919..957` hung in deep traceback generation.
- The skip branch emitted `"... (skipping N levels)"` but left `limit2show`
  at zero, so later iterations kept taking the skip branch and could move the
  level backward when `N` became negative.
- Fix: after the skip branch, set `limit2show = LEVELS2`.
- Lesson: when a truncation loop has two display windows, the skip transition
  must explicitly switch to the second window.

Next failure: stripped debug info:

- `string.dump(load(prog), true)` strips debug metadata.
- `debug.getupvalue` / `setupvalue` should return `"(no name)"` when the
  upvalue slot exists but its debug name is absent.
- Missing proto source should behave like `"=?"`, whose short source is `"?"`,
  not `[string ""]`.
- Fixes: use `"(no name)"` for absent Lua upvalue names and `"=?"` for absent
  Lua proto source in debug info.

Final verification:

```sh
RUSTFLAGS='-Awarnings' TEST_TIMEOUT_S=45 ./harness/run_official_test.sh reference/lua-c/testes/db.lua
# PASS reference/lua-c/testes/db.lua
```

Focused repros that should stay useful:

- `harness/repro/db_gap_line_repro.lua`
- `harness/repro/db_vararg_getlocal_repro.lua`
- `harness/repro/db_nonregistered_temp_repro.lua`
- `harness/repro/db_open_upvalue_repro.lua`
- `harness/repro/db_count_hook_repro.lua`
- `harness/repro/db_coroutine_traceback_repro.lua`
- `harness/repro/db_hook_gc_getinfo_abort_repro.lua`
- `harness/repro/db_hook_gc_gethook_getinfo_min_repro.lua`
- `harness/repro/db_hook_disable_coroutine_traceback_repro.lua`
- `harness/repro/db_hook_line_count_coroutine_traceback_repro.lua`
- `harness/repro/db_hook_getlocal_coroutine_traceback_repro.lua`

## GC Root Graph And Weak-Table Ladder

Use this when a GC test looks fixed in isolation but fails after another test,
inside `all.lua`, or only after `string.dump`/`load`. The main trick is to
separate three concerns that tend to collapse together: reachability, registry
bookkeeping, and table-shape invariants.

### Working Rules

1. **Do not treat registries as roots**

   Lists like coroutine registries, weak-table registries, long-string
   accounting lists, and cross-thread upvalue maps are ownership/accounting
   structures. They should be cleaned after a mark phase, not traced
   unconditionally. If tracing the list fixes a crash, assume it is hiding a
   lifetime bug until proven otherwise.

2. **After unrooting a registry, add post-mark cleanup**

   Removing an accidental root usually exposes stale registry rows. The safe
   pattern is:

   - mark real roots;
   - trace only registry entries whose public handle was marked;
   - after mark, retain only those live registry IDs;
   - drop side maps keyed by dead IDs.

3. **Close escaped open upvalues before sweeping dead coroutines**

   A closure can survive after its coroutine wrapper dies. If that closure has
   an open upvalue pointing into the dead coroutine stack, the upvalue must be
   closed while the coroutine stack is still readable. Close only open upvalues
   whose `UpVal` object was marked by a surviving closure; then trace the
   closed value and drain the gray queue again.

4. **Finalizers run with GC internally stopped**

   Lua C sets the internal GC-stop bit while running `__gc`. Nested
   `collectgarbage()` from inside a finalizer returns no result. Reproduce this
   with a tiny finalizer toy before changing broad collection behavior.

5. **Generational step can need weak cleanup without sweeping**

   If object ages are not implemented, do not fake a generational step with a
   full collection. A mark-only pass with the weak-table post-mark hook is much
   safer: it clears all-weak entries that the official test expects while
   avoiding collection of suspended coroutine state.

6. **Weak-table deletion must preserve table chains**

   Clearing a hash node's key/value is not enough if it is part of a collision
   chain. Either patch the predecessor's `next`, or move the successor into the
   removed head slot and rewrite the successor's relative `next` offset. A dead
   long-string key left in a chain can make `next(t, k)` find a stale equal
   string and return the same live entry again.

7. **`all.lua` is a bytecode oracle**

   The meta-runner round-trips many files through `string.dump`/`load`. If a
   test passes standalone but fails only in `all.lua`, check dumped constants,
   line info, locals, upvalue names, and stripped-source behavior. In this
   case, `LuaValue::full_type_tag()` wrote base tags, so a float constant was
   reloaded as an integer containing the f64 bit pattern.

### Fast Toys

Finalizer nested-GC behavior:

```sh
target/debug/lua-rs -e 'local res=true; setmetatable({}, {__gc=function() res=collectgarbage() end}); collectgarbage(); print(res == nil, tostring(res))'
```

Generational weak cleanup:

```sh
target/debug/lua-rs -e 'collectgarbage("generational"); local t=setmetatable({}, {__mode="kv"}); collectgarbage(); t[1]={10}; print("step1", collectgarbage("step",0), t[1] and t[1][1]); print("step2", collectgarbage("step",0), t[1] and t[1][1])'
```

Escaped coroutine upvalue:

```sh
target/debug/lua-rs -e 'local x=coroutine.wrap(function() local a=10; local function f() a=a+10; return a end; while true do a=a+1; coroutine.yield(f) end end); local f=x(); print(f(), x()(), x()==f); x=nil; collectgarbage(); print(f(), f())'
```

Dump/load constant tags:

```sh
target/debug/lua-rs -e 'local f=assert(load([[return true, false, 3, 3.5, 2^24 - 1, "abc"]])); print(assert(load(string.dump(f)))())'
```

Weak long-string chain cleanup:

```sh
target/debug/lua-rs -e 'collectgarbage(); collectgarbage(); local a=setmetatable({}, {__mode="kv"}); local ka=string.rep("a", 2^22); local kb=string.rep("b", 2^22); a[ka]=25; a[kb]={}; a[{}]=14; collectgarbage(); local k,v=next(a); print(k and #k, v, next(a,k)); print(a[string.rep("b", 2^22)] == nil); a[k]=nil; k=nil; collectgarbage(); print(next(a))'
```

### Final Verification

Run the focused gates first:

```sh
RUSTFLAGS='-Awarnings' TEST_TIMEOUT_S=60 ./harness/run_official_test.sh reference/lua-c/testes/gc.lua
RUSTFLAGS='-Awarnings' TEST_TIMEOUT_S=60 ./harness/run_official_test.sh reference/lua-c/testes/gengc.lua
RUSTFLAGS='-Awarnings' TEST_TIMEOUT_S=60 ./harness/run_official_test.sh reference/lua-c/testes/coroutine.lua
RUSTFLAGS='-Awarnings' TEST_TIMEOUT_S=45 ./harness/run_official_test.sh reference/lua-c/testes/db.lua
```

Then run the bytecode/meta-runner gate:

```sh
RUSTFLAGS='-Awarnings' TEST_TIMEOUT_S=90 ./harness/run_official_all.sh
```

Expected result at this checkpoint: `44/44`, 0 failures, 0 timeouts.
