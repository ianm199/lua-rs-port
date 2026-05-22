-- harness preamble: emulate the globals lua-c testes/all.lua sets
_soft = true
_port = true
_nomsg = true
_U = false
arg = arg or {}
_G = _G or _ENV
if _VERSION == nil then _VERSION = "Lua 5.4" end

_soft = true

print("test1: init parameter in gmatch")
do   -- init parameter in gmatch
  local s = 0
  for k in string.gmatch("10 20 30", "%d+", 3) do
    s = s + tonumber(k)
  end
  assert(s == 50)
  print("test1.1 passed")

  s = 0
  for k in string.gmatch("11 21 31", "%d+", -4) do
    s = s + tonumber(k)
  end
  assert(s == 32)
  print("test1.2 passed")

  -- there is an empty string at the end of the subject
  s = 0
  for k in string.gmatch("11 21 31", "%w*", 9) do
    s = s + 1
  end
  assert(s == 1)
  print("test1.3 passed")

  -- there are no empty strings after the end of the subject
  s = 0
  for k in string.gmatch("11 21 31", "%w*", 10) do
    s = s + 1
  end
  assert(s == 0)
  print("test1.4 passed")
end

print("test2: %f frontier")
assert(string.gsub("aaa aa a aaa a", "%f[%w]a", "x") == "xaa xa x xaa x")
print("test2.1 passed")
assert(string.gsub("[[]] [][] [[[[", "%f[[].", "x") == "x[]] x]x] x[[[")
print("test2.2 passed")
assert(string.gsub("01abc45de3", "%f[%d]", ".") == ".01abc.45de.3")
print("test2.3 passed")
assert(string.gsub("01abc45 de3x", "%f[%D]%w", ".") == "01.bc45 de3.")
print("test2.4 passed")
assert(string.gsub("function", "%f[\1-\255]%w", ".") == ".unction")
print("test2.5 passed")
assert(string.gsub("function", "%f[^\1-\255]", ".") == "function.")
print("test2.6 passed")
