-- harness preamble: emulate the globals lua-c testes/all.lua sets
_soft = true
_port = true
_nomsg = true
_U = false
arg = arg or {}
_G = _G or _ENV
if _VERSION == nil then _VERSION = "Lua 5.4" end

local s1 = string.rep("a", 300); local s2 = string.rep("a", 300)
print('s1 p=', string.format("%p", s1))
print('s2 p=', string.format("%p", s2))
print('s1==s2 by p?', string.format("%p", s1) == string.format("%p", s2))
local short1 = string.rep("a", 10)
local short2 = string.rep("aa", 5)
print('short1 p=', string.format("%p", short1))
print('short2 p=', string.format("%p", short2))
print('short1==short2 by p?', string.format("%p", short1) == string.format("%p", short2))
