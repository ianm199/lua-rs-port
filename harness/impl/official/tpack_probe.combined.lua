-- harness preamble: emulate the globals lua-c testes/all.lua sets
_soft = true
_port = true
_nomsg = true
_U = false
arg = arg or {}
_G = _G or _ENV
if _VERSION == nil then _VERSION = "Lua 5.4" end

local pack = string.pack
local packsize = string.packsize
local unpack = string.unpack
local NB = 16
local function checkerror (msg, f, ...)
  local status, err = pcall(f, ...)
  if not (not status and string.find(err, msg)) then
    print("FAIL_MSG:", msg, "status:", status, "err:", err)
    error("STOP")
  end
end

checkerror("out of limits", pack, "i0", 0)
checkerror("out of limits", pack, "i" .. NB + 1, 0)
checkerror("out of limits", pack, "!" .. NB + 1, 0)
checkerror("%(17%) out of limits %[1,16%]", pack, "Xi" .. NB + 1)
checkerror("invalid format option 'r'", pack, "i3r", 0)
checkerror("16%-byte integer", unpack, "i16", string.rep('\3', 16))
checkerror("not power of 2", pack, "!4i3", 0);
checkerror("missing size", pack, "c", "")
checkerror("variable%-length format", packsize, "s")
checkerror("variable%-length format", packsize, "z")
checkerror("invalid format", packsize, "c1" .. string.rep("0", 40))
print("done block 1")

if packsize("i") == 4 then
  local s = string.rep("c268435456", 2^3)
  checkerror("too large", packsize, s)
  s = string.rep("c268435456", 2^3 - 1) .. "c268435455"
  assert(packsize(s) == 0x7fffffff)
end
print("done block 2")

local sizeLI = packsize("j")
for i = 1, sizeLI - 1 do
  local umax = (1 << (i * 8)) - 1
  local max = umax >> 1
  local min = ~max
  checkerror("overflow", pack, "<I" .. i, -1)
  checkerror("overflow", pack, "<I" .. i, min)
  checkerror("overflow", pack, ">I" .. i, umax + 1)

  checkerror("overflow", pack, ">i" .. i, umax)
  checkerror("overflow", pack, ">i" .. i, max + 1)
  checkerror("overflow", pack, "<i" .. i, min - 1)

  assert(unpack(">i" .. i, pack(">i" .. i, max)) == max)
  assert(unpack("<i" .. i, pack("<i" .. i, min)) == min)
  assert(unpack(">I" .. i, pack(">I" .. i, umax)) == umax)
end
print("done block 3")
