-- canary_q_coro_traceback_root — issue #140 bug A.
--
-- debug.traceback(co) borrows the target coroutine's state for the whole
-- traceback while pushing per-frame strings on the caller (allocations).
-- A collect triggered by one of those allocations could not trace the
-- borrowed coroutine, so the closures in its suspended frames were swept
-- and the traceback's next frame query dereferenced freed memory.
--
-- Deterministic under the stress+quarantine battery (LUA_RS_GC_STRESS=1
-- LUA_RS_GC_QUARANTINE=1, debug build): the first in-traceback allocation
-- checkpoint collects while the borrow is held. Fails (panic) on commits
-- before the RootedThreadBorrow fix; passes after.

local co = coroutine.create(function(seed)
  local function inner(depth)
    if depth == 0 then
      coroutine.yield("suspended")
      return "resumed"
    end
    local pad = { tostring(depth), depth + seed }
    return inner(depth - 1), pad[1]
  end
  return inner(6)
end)

assert(coroutine.resume(co, 1))
assert(coroutine.status(co) == "suspended")

for i = 1, 40 do
  local msg = "m" .. string.rep("x", i % 9) .. i
  local tb = debug.traceback(co, msg, 0)
  assert(type(tb) == "string")
  assert(tb:find("stack traceback:", 1, true), "traceback body missing")
  local lvl1 = debug.getinfo(co, 0, "Slnf")
  assert(type(lvl1) == "table")
end

assert(coroutine.resume(co))
assert(coroutine.status(co) == "dead")
print("OK")
