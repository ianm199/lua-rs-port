local debug = require "debug"

print"testing debug library and debug information"

local a = {}
local L = nil
local glob = 1
local oldglob = glob

debug.sethook(function(e, l)
  collectgarbage()
  local _, m, c = debug.gethook()
  assert(m == "crl" and c == 0)
  if e == "line" then
    if glob ~= oldglob then
      L = l - 1
      oldglob = glob
    end
  elseif e == "call" then
    local f = debug.getinfo(2, "f").func
    a[f] = 1
  else
    assert(e == "return")
  end
end, "crl")

debug.sethook(nil)

local function checktraceback(co, p, level)
  local tb = debug.traceback(co, nil, level)
  local i = 0
  for l in string.gmatch(tb, "[^\n]+\n?") do
    assert(i == 0 or string.find(l, p[i]))
    i = i + 1
  end
  assert(p[i] == nil)
end

local function f(n)
  if n > 0 then f(n - 1)
  else coroutine.yield() end
end

local co = coroutine.create(f)
coroutine.resume(co, 3)
checktraceback(co, {"yield", "db_hook_disable_coroutine_traceback_repro.lua", "db_hook_disable_coroutine_traceback_repro.lua", "db_hook_disable_coroutine_traceback_repro.lua", "db_hook_disable_coroutine_traceback_repro.lua"})
