-- Minimal reproducer focused on the current coroutine + weak-table UB path.
-- Keeps only the pieces that stress tbclist + close + weak-table interactions.

local function func2close(f)
  return setmetatable({}, { __close = f })
end

-- Weak table containing a coroutine wrapper used by the finalizer/close chain.
local C = setmetatable({}, { __mode = "kv" })
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

-- Infinite recursion through coroutine.wrap (another old trigger path).
local co1, co2
co1 = coroutine.create(function()
  return co2()
end)
co2 = coroutine.wrap(function()
  assert(coroutine.status(co1) == "normal")
  assert(not coroutine.resume(co1))
  coroutine.yield(3)
end)
local a = function(y)
  return coroutine.wrap(y)(y)
end
pcall(a, a)
