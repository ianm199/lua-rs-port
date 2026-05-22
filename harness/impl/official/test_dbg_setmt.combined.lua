-- harness preamble: emulate the globals lua-c testes/all.lua sets
_soft = true
_port = true
_nomsg = true
_U = false
arg = arg or {}
_G = _G or _ENV
if _VERSION == nil then _VERSION = "Lua 5.4" end

local debug = require'debug'
local mt = {__index = function(a,b) return a+b end,
            __len = function(x) return math.floor(x) end}
print('before debug.setmetatable')
debug.setmetatable(10, mt)
print('after debug.setmetatable')
print(getmetatable(-2) == mt)
