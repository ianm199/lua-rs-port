local function checktraceback(co, patterns, level)
  local tb = debug.traceback(co, nil, level)
  assert(type(tb) == "string", type(tb))
  local i = 0
  for line in string.gmatch(tb, "[^\n]+\n?") do
    assert(i == 0 or string.find(line, patterns[i]), line)
    i = i + 1
  end
  assert(patterns[i] == nil)
end

local function f(n)
  if n > 0 then
    f(n - 1)
  else
    coroutine.yield()
  end
end

local co = coroutine.create(f)
coroutine.resume(co, 3)

checktraceback(co, {"yield", "db_coroutine_traceback_repro.lua", "db_coroutine_traceback_repro.lua", "db_coroutine_traceback_repro.lua", "db_coroutine_traceback_repro.lua"})
checktraceback(co, {"db_coroutine_traceback_repro.lua", "db_coroutine_traceback_repro.lua", "db_coroutine_traceback_repro.lua", "db_coroutine_traceback_repro.lua"}, 1)
checktraceback(co, {"db_coroutine_traceback_repro.lua", "db_coroutine_traceback_repro.lua", "db_coroutine_traceback_repro.lua"}, 2)
checktraceback(co, {"db_coroutine_traceback_repro.lua"}, 4)
checktraceback(co, {}, 40)

co = coroutine.create(function(x)
  local a = 1
  coroutine.yield(debug.getinfo(1, "l"))
  coroutine.yield(debug.getinfo(1, "l").currentline)
  return a
end)

local tr = {}
local hook = function(_, line)
  if line then tr[#tr + 1] = line end
end
debug.sethook(co, hook, "lcr")

local _, line_info = coroutine.resume(co, 10)
local info = debug.getinfo(co, 1, "lfLS")
assert(info.currentline == line_info.currentline)
assert(info.activelines[info.currentline])
assert(type(info.func) == "function")

local name, value = debug.getlocal(co, 1, 1)
assert(name == "x" and value == 10)
name, value = debug.getlocal(co, 1, 2)
assert(name == "a" and value == 1)
debug.setlocal(co, 1, 2, "hi")
assert(debug.gethook(co) == hook)
