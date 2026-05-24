a = {}; local L = nil
local glob = 1
local oldglob = glob

debug.sethook(function (e,l)
  collectgarbage()   -- force GC during a hook
  local f, m, c = debug.gethook()
  assert(m == 'crl' and c == 0)
  if e == "line" then
    if glob ~= oldglob then
      L = l-1   -- get the first line where "glob" has changed
      oldglob = glob
    end
  elseif e == "call" then
      local f = debug.getinfo(2, "f").func
      a[f] = 1
  else assert(e == "return")
  end
end, "crl")


function f(a,b)
  collectgarbage()
  local _, x = debug.getlocal(1, 1)
  local _, y = debug.getlocal(1, 2)
  assert(x == a and y == b)
  assert(debug.setlocal(2, 3, "pera") == "AA".."AA")
  assert(debug.setlocal(2, 4, "manga") == "B")
  x = debug.getinfo(2)
  assert(x.func == g and x.what == "Lua")
  glob = glob+1
  assert(debug.getinfo(1, "l").currentline == L+1)
  assert(debug.getinfo(1, "l").currentline == L+2)
end

function foo()
  glob = glob+1
  assert(debug.getinfo(1, "l").currentline == L+1)
end; foo()  -- set L
-- check line counting inside strings and empty lines

local _ = 'alo\
alo' .. [[

]]
--[[
]]
assert(debug.getinfo(1, "l").currentline == L+11)  -- check count of lines


function g (...)
  local arg = {...}
  do local a,b,c; a=math.sin(40); end
  local feijao
  local AAAA,B = "xuxu", "abacate"
  f(AAAA,B)
  assert(AAAA == "pera" and B == "manga")
  do
     local B = 13
     local x,y = debug.getlocal(1,5)
     assert(x == 'B' and y == 13)
  end
end

g()


assert(a[f] and a[g] and a[assert] and a[debug.getlocal] and not a[print])


-- tests for manipulating non-registered locals (C and Lua temporaries)

local n, v = debug.getlocal(0, 1)
assert(v == 0 and n == "(C temporary)")
local n, v = debug.getlocal(0, 2)
assert(v == 2 and n == "(C temporary)")
assert(not debug.getlocal(0, 3))
assert(not debug.getlocal(0, 0))

function f()
  assert(select(2, debug.getlocal(2,3)) == 1)
  assert(not debug.getlocal(2,4))
  debug.setlocal(2, 3, 10)
  return 20
end

function g(a,b) return (a+1) + f() end

assert(g(0,0) == 30)

_G.f, _G.g = nil

debug.sethook(nil);
assert(not debug.gethook())

local function checktraceback (co, p, level)
  local tb = debug.traceback(co, nil, level)
  local i = 0
  for l in string.gmatch(tb, "[^\n]+\n?") do
    assert(i == 0 or string.find(l, p[i]))
    i = i+1
  end
  assert(p[i] == nil)
end


local function f (n)
  if n > 0 then f(n-1)
  else coroutine.yield() end
end

local co = coroutine.create(f)
coroutine.resume(co, 3)
checktraceback(co, {"yield", "db_hook_getlocal_coroutine_traceback_repro.lua", "db_hook_getlocal_coroutine_traceback_repro.lua", "db_hook_getlocal_coroutine_traceback_repro.lua", "db_hook_getlocal_coroutine_traceback_repro.lua"})
checktraceback(co, {"db_hook_getlocal_coroutine_traceback_repro.lua", "db_hook_getlocal_coroutine_traceback_repro.lua", "db_hook_getlocal_coroutine_traceback_repro.lua", "db_hook_getlocal_coroutine_traceback_repro.lua"}, 1)
checktraceback(co, {"db_hook_getlocal_coroutine_traceback_repro.lua", "db_hook_getlocal_coroutine_traceback_repro.lua", "db_hook_getlocal_coroutine_traceback_repro.lua"}, 2)
checktraceback(co, {"db_hook_getlocal_coroutine_traceback_repro.lua"}, 4)
checktraceback(co, {}, 40)
