-- harness preamble: emulate the globals lua-c testes/all.lua sets
_soft = true
_port = true
_nomsg = true
_U = false
arg = arg or {}
_G = _G or _ENV
if _VERSION == nil then _VERSION = "Lua 5.4" end

-- isolate repeat-break with locals
print("start")

local function f1()
  local x = 1;
  repeat
    x = 10
    break
  until x>=12
  return x
end
print("f1 =", f1())

local function f2(b)
  local x = 1;
  repeat
    local a;
    if b==1 then local b=1; x=10; break end
  until x>=12;
  return x;
end
print("f2(1) =", f2(1))

local function f3(b)
  local x = 1;
  repeat
    local a;
    if b==1 then
      local b=1;
      x=10;
      break
    elseif b==2 then x=20; break
    end
  until x>=12;
  return x;
end
print("f3(1) =", f3(1))

print("end")
