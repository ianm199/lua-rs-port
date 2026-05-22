-- harness preamble: emulate the globals lua-c testes/all.lua sets
_soft = true
_port = true
_nomsg = true
_U = false
arg = arg or {}
_G = _G or _ENV
if _VERSION == nil then _VERSION = "Lua 5.4" end

-- isolate f(b) with repeat-until-break
print("start")

function f(b)
  local x = 1;
  repeat
    local a;
    if b==1 then local b=1; x=10; break
    elseif b==2 then x=20; break;
    elseif b==3 then x=30;
    else local a,b,c,d=math.sin(1); x=x+1;
    end
  until x>=12;
  return x;
end;
print("defined f")

local r1 = f(1)
print("f(1) =", r1)
local r2 = f(2)
print("f(2) =", r2)
local r3 = f(3)
print("f(3) =", r3)
local r4 = f(4)
print("f(4) =", r4)
print("end")
