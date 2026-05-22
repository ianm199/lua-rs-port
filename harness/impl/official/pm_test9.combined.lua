-- harness preamble: emulate the globals lua-c testes/all.lua sets
_soft = true
_port = true
_nomsg = true
_U = false
arg = arg or {}
_G = _G or _ENV
if _VERSION == nil then _VERSION = "Lua 5.4" end

_soft = true
print("test1: \\0 in patterns")
assert(string.match("ab\0\1\2c", "[\0-\2]+") == "\0\1\2")
print("test1.1 passed")
assert(string.match("ab\0\1\2c", "[\0-\0]+") == "\0")
print("test1.2 passed")
assert(string.find("b$a", "$\0?") == 2)
print("test1.3 passed")
assert(string.find("abc\0efg", "%\0") == 4)
print("test1.4 passed")
assert(string.match("abc\0efg\0\1e\1g", "%b\0\1") == "\0efg\0\1e\1")
print("test1.5 passed")
assert(string.match("abc\0\0\0", "%\0+") == "\0\0\0")
print("test1.6 passed")
assert(string.match("abc\0\0\0", "%\0%\0?") == "\0\0")
print("test1.7 passed")

print("test2: magic char after \\0")
assert(string.find("abc\0\0","\0.") == 4)
print("test2.1 passed")
assert(string.find("abcx\0\0abc\0abc","x\0\0abc\0a.") == 4)
print("test2.2 passed")

print("test3: reuse of original string in gsub")
do   -- test reuse of original string in gsub
  local s = string.rep("a", 100)
  local r = string.gsub(s, "b", "c")   -- no match
  assert(string.format("%p", s) == string.format("%p", r))
  print("test3.1 passed")

  r = string.gsub(s, ".", {x = "y"})   -- no substitutions
  assert(string.format("%p", s) == string.format("%p", r))
  print("test3.2 passed")

  local count = 0
  r = string.gsub(s, ".", function (x)
                            assert(x == "a")
                            count = count + 1
                            return nil    -- no substitution
                          end)
  r = string.gsub(r, ".", {b = 'x'})   -- "a" is not a key; no subst.
  assert(count == 100)
  assert(string.format("%p", s) == string.format("%p", r))
  print("test3.3 passed")

  count = 0
  r = string.gsub(s, ".", function (x)
                            assert(x == "a")
                            count = count + 1
                            return x    -- substitution...
                          end)
  assert(count == 100)
  -- no reuse in this case
  assert(r == s and string.format("%p", s) ~= string.format("%p", r))
  print("test3.4 passed")
end

print('OK')
