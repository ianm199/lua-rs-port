-- harness preamble: emulate the globals lua-c testes/all.lua sets
_soft = true
_port = true
_nomsg = true
_U = false
arg = arg or {}
_G = _G or _ENV
if _VERSION == nil then _VERSION = "Lua 5.4" end

local lim = 20
local t = {[1] = 20.3}

local function f()
  print("inside f, lim+0.3:", lim+0.3)
  print("inside f, t[1]:", t[1])
  print("inside f, t[1] == lim+0.3:", t[1] == lim + 0.3)
  print("inside f, t[1] == 20.3:", t[1] == 20.3)
  local x = lim + 0.3
  print("inside f, x:", x, "t[1]==x:", t[1] == x)
end
f()
