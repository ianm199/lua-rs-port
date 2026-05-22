-- harness preamble: emulate the globals lua-c testes/all.lua sets
_soft = true
_port = true
_nomsg = true
_U = false
arg = arg or {}
_G = _G or _ENV
if _VERSION == nil then _VERSION = "Lua 5.4" end

print("start")

local x = 1
print("step1")
repeat
  print("inside repeat")
  x = 10
  break
until x >= 12
print("after repeat, x=", x)
print("end")
