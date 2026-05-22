-- harness preamble: emulate the globals lua-c testes/all.lua sets
_soft = true
_port = true
_nomsg = true
_U = false
arg = arg or {}
_G = _G or _ENV
if _VERSION == nil then _VERSION = "Lua 5.4" end

print("before assert")
local result = "2" + 1
print("result=", result, type(result))
assert(result == 3)
print("after assert")

print("now try the inline form")
assert("2" + 1 == 3)
print("done")
