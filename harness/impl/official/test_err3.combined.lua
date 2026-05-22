-- harness preamble: emulate the globals lua-c testes/all.lua sets
_soft = true
_port = true
_nomsg = true
_U = false
arg = arg or {}
_G = _G or _ENV
if _VERSION == nil then _VERSION = "Lua 5.4" end

local function foo()
  error("@Y")
end
local st, msg = xpcall(foo, debug.traceback)
print("msg:", msg)
print("---")

local function bar()
  error("@Y")
end
local st2, msg2 = pcall(bar)
print("msg2:", msg2)
