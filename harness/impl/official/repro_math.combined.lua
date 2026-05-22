-- harness preamble: emulate the globals lua-c testes/all.lua sets
_soft = true
_port = true
_nomsg = true
_U = false
arg = arg or {}
_G = _G or _ENV
if _VERSION == nil then _VERSION = "Lua 5.4" end

local function checkerror (msg, f, ...)
  local s, err = pcall(f, ...)
  print("pcall_returned:", s, err)
  assert(not s and string.find(err, msg))
end

print("about to call checkerror with math.ceil(print)")
checkerror("number expected", math.ceil, print)
print("DONE")
