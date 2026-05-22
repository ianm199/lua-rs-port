-- harness preamble: emulate the globals lua-c testes/all.lua sets
_soft = true
_port = true
_nomsg = true
_U = false
arg = arg or {}
_G = _G or _ENV
if _VERSION == nil then _VERSION = "Lua 5.4" end

_soft = true
_port = true
_nomsg = true
_U = false
arg = arg or {}
_G = _G or _ENV
if _VERSION == nil then _VERSION = "Lua 5.4" end

print("testing numbers and math lib")

local minint <const> = math.mininteger
local maxint <const> = math.maxinteger

local intbits <const> = math.floor(math.log(maxint, 2) + 0.5) + 1
assert((1 << intbits) == 0)

assert(minint == 1 << (intbits - 1))
assert(maxint == minint - 1)

local floatbits = 24
do
  local p = 2.0^floatbits
  while p < p + 1.0 do
    p = p * 2.0
    floatbits = floatbits + 1
  end
end

local function isNaN (x)
  return (x ~= x)
end

assert(isNaN(0/0))
assert(not isNaN(1/0))


do
  local x = 2.0^floatbits
  assert(x > x - 1.0 and x == x + 1.0)

  print(string.format("%d-bit integers, %d-bit (mantissa) floats",
                       intbits, floatbits))
end

assert(math.type(0) == "integer" and math.type(0.0) == "float"
       and not math.type("10"))


local function checkerror (msg, f, ...)
  local s, err = pcall(f, ...)
  assert(not s and string.find(err, msg))
end

local msgf2i = "number.* has no integer representation"

local function eq (a,b,limit)
  if not limit then
    if floatbits >= 50 then limit = 1E-11
    else limit = 1E-5
    end
  end
  return a == b or math.abs(a-b) <= limit
end


local function eqT (a,b)
  return a == b and math.type(a) == math.type(b)
end

if floatbits < intbits then
  print("checkpoint A: before testing order block")
  print("testing order (floats cannot represent all integers)")
  local fmax = 2^floatbits
  local ifmax = fmax | 0
  assert(fmax < ifmax + 1)
  assert(fmax - 1 < ifmax)
  assert(-(fmax - 1) > -ifmax)
  assert(not (fmax <= ifmax - 1))
  assert(-fmax > -(ifmax + 1))
  assert(not (-fmax >= -(ifmax - 1)))

  assert(fmax/2 - 0.5 < ifmax//2)
  assert(-(fmax/2 - 0.5) > -ifmax//2)

  assert(maxint < 2^intbits)
  assert(minint > -2^intbits)
  assert(maxint <= 2^intbits)
  assert(minint >= -2^intbits)
  print("checkpoint B: after testing order block")
end

do
  local NaN <const> = 0/0
  assert(not (NaN < 0))
  assert(not (NaN > minint))
  assert(not (NaN <= -9))
  assert(not (NaN <= maxint))
  assert(not (NaN < maxint))
  assert(not (minint <= NaN))
  assert(not (minint < NaN))
  assert(not (4 <= NaN))
  assert(not (4 < NaN))
end
print("checkpoint C: after NaN tests")


-- avoiding errors at compile time
local function checkcompt (msg, code)
  checkerror(msg, assert(load(code)))
end
print("checkpoint D: before checkcompt calls")
checkcompt("divide by zero", "return 2 // 0")
print("checkpoint E: after divide by zero")
checkcompt(msgf2i, "return 2.3 >> 0")
print("checkpoint F: after 2.3>>0")
checkcompt(msgf2i, ("return 2.0^%d & 1"):format(intbits - 1))
print("checkpoint G: after 2.0^N & 1")
checkcompt("field 'huge'", "return math.huge << 1")
print("checkpoint H: after math.huge << 1")
checkcompt(msgf2i, ("return 1 | 2.0^%d"):format(intbits - 1))
print("checkpoint I: after 1|2.0^N")
checkcompt(msgf2i, "return 2.3 ~ 0.0")
print("checkpoint J: after 2.3~0.0")
