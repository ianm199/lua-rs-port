-- harness preamble: emulate the globals lua-c testes/all.lua sets
_soft = true
_port = true
_nomsg = true
_U = false
arg = arg or {}
_G = _G or _ENV
if _VERSION == nil then _VERSION = "Lua 5.4" end

_soft = true

print("test1: %f basic")
assert(string.find("a", "%f[a]") == 1)
print("test1.1 passed")
assert(string.find("a", "%f[^%z]") == 1)
print("test1.2 passed")
assert(string.find("a", "%f[^%l]") == 2)
print("test1.3 passed")
assert(string.find("aba", "%f[a%z]") == 3)
print("test1.4 passed")
assert(string.find("aba", "%f[%z]") == 4)
print("test1.5 passed")
assert(not string.find("aba", "%f[%l%z]"))
print("test1.6 passed")
assert(not string.find("aba", "%f[^%l%z]"))
print("test1.7 passed")

print("test2: complex %f")
local i, e = string.find(" alo aalo allo", "%f[%S].-%f[%s].-%f[%S]")
assert(i == 2 and e == 5)
print("test2.1 passed")
local k = string.match(" alo aalo allo", "%f[%S](.-%f[%s].-%f[%S])")
assert(k == 'alo ')
print("test2.2 passed")

print("test3: gmatch %f")
local a = {1, 5, 9, 14, 17,}
for k in string.gmatch("alo alo th02 is 1hat", "()%f[%w%d]") do
  assert(table.remove(a, 1) == k)
end
assert(#a == 0)
print("test3 passed")

print("test4: malformed patterns")
local function malform (p, m)
  m = m or "malformed"
  local r, msg = pcall(string.find, "a", p)
  assert(not r and string.find(msg, m))
end

malform("(.", "unfinished capture")
print("test4.1 passed")
malform(".)", "invalid pattern capture")
print("test4.2 passed")
malform("[a")
print("test4.3 passed")
malform("[]")
print("test4.4 passed")
malform("[^]")
print("test4.5 passed")
malform("[a%]")
print("test4.6 passed")
malform("[a%")
print("test4.7 passed")
malform("%b")
print("test4.8 passed")
malform("%ba")
print("test4.9 passed")
malform("%")
print("test4.10 passed")
malform("%f", "missing")
print("test4.11 passed")
