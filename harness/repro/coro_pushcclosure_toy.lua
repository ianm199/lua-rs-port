-- Minimized reproducer for the current coroutine/table-iteration UB crash.
-- Run with:
--   RUST_BACKTRACE=1 ./target/debug/lua-rs harness/repro/coro_pushcclosure_toy.lua

local function func2close(f)
  return setmetatable({}, {__close = f})
end

-- Keep this block: it creates a weak-table edge into a soon-to-be-collected
-- coroutine wrapper and is needed to get the trace in this build to hit.
local C = {}
setmetatable(C, {__mode = "kv"})
local x = coroutine.wrap(function()
  return 1
end)
C[1] = x
local f = x()
f = tostring(f)
x = nil
collectgarbage()

-- 5.4.1 close-reset pattern with closure over the wrapped coroutine.
do
  local co
  co = coroutine.wrap(function()
    local x <close> = func2close(function()
      return pcall(co)
    end)
    error(111)
  end)
  local _, _ = pcall(co)
  local _, _ = pcall(co)
end

-- Infinite recursion through coroutine.wrap.
local co1, co2
co1 = coroutine.create(function()
  return co2()
end)
co2 = coroutine.wrap(function()
  assert(coroutine.status(co1) == "normal")
  assert(not coroutine.resume(co1))
  coroutine.yield(3)
end)

local a = function(a)
  return coroutine.wrap(a)(a)
end
pcall(a, a)
