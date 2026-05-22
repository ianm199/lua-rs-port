-- harness preamble: emulate the globals lua-c testes/all.lua sets
_soft = true
_port = true
_nomsg = true
_U = false
arg = arg or {}
_G = _G or _ENV
if _VERSION == nil then _VERSION = "Lua 5.4" end

print "testing closures"

local A,B = 0,{g=10}
local function f(x)
  local a = {}
  for i=1,1000 do
    local y = 0
    do
      a[i] = function () B.g = B.g+1; y = y+x; return y+A end
    end
  end
  local dummy = function () return a[A] end
  collectgarbage()
  A = 1; assert(dummy() == a[1]); A = 0;
  assert(a[1]() == x)
  assert(a[3]() == x)
  collectgarbage()
  assert(B.g == 12)
  return a
end

print("calling f(10)")
local a = f(10)
print("f(10) returned")
-- force a GC in this level
local x = {[1] = {}}   -- to detect a GC
print("set x")
setmetatable(x, {__mode = 'kv'})
print("set meta")
local count = 0
while x[1] do   -- repeat until GC
  local a = A..A..A..A  -- create garbage
  A = A+1
  count = count + 1
  if count > 5000 then print("loop too long, count=", count); break end
end
print("after gc loop, count=", count, "A=", A)
assert(a[1]() == 20+A)
print("a1 ok")
assert(a[1]() == 30+A)
print("a1b ok")
