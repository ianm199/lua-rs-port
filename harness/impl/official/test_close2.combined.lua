-- harness preamble: emulate the globals lua-c testes/all.lua sets
_soft = true
_port = true
_nomsg = true
_U = false
arg = arg or {}
_G = _G or _ENV
if _VERSION == nil then _VERSION = "Lua 5.4" end

local function func2close (f)
  return setmetatable({}, {__close = f})
end

local function foo (...)
  do
    local x1 <close> =
      func2close(function (self, msg)
        assert(string.find(msg, "@X"))
        error("@Y")
      end)
    local x123 <close> =
      func2close(function (_, msg)
        assert(msg == nil)
        error("@X")
      end)
  end
end

local st, msg = xpcall(foo, debug.traceback)
print("---msg---")
print(msg)
print("---end---")
print("match:", string.match(msg, "^[^ ]* @Y"))
