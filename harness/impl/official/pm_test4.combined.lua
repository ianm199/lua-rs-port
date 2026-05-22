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

local function checkerror (msg, f, ...)
  local s, err = pcall(f, ...)
  assert(not s and string.find(err, msg))
end

print("test1: list-style replacement table")
local t = {"apple", "orange", "lime"; n=0}
assert(string.gsub("x and x and x", "x", function () t.n=t.n+1; return t[t.n] end)
        == "apple and orange and lime")
print("test1 passed")

print("test2: count callback")
t = {n=0}
string.gsub("first second word", "%w%w*", function (w) t.n=t.n+1; t[t.n] = w end)
assert(t[1] == "first" and t[2] == "second" and t[3] == "word" and t.n == 3)
print("test2 passed")

print("test3: max-replacements")
t = {n=0}
assert(string.gsub("first second word", "%w+",
         function (w) t.n=t.n+1; t[t.n] = w end, 2) == "first second word")
assert(t[1] == "first" and t[2] == "second" and t[3] == undef)
print("test3 passed")

print("test4: checkerror table replacement")
checkerror("invalid replacement value %(a table%)",
            string.gsub, "alo", ".", {a = {}})
print("test4 passed")

print("test5: invalid capture index %%2")
checkerror("invalid capture index %%2", string.gsub, "alo", ".", "%2")
print("test5 passed")

print("test6: invalid capture index %%0")
checkerror("invalid capture index %%0", string.gsub, "alo", "(%0)", "a")
print("test6 passed")

print("test7: invalid capture index %%1")
checkerror("invalid capture index %%1", string.gsub, "alo", "(%1)", "a")
print("test7 passed")

print("test8: invalid use of %%")
checkerror("invalid use of '%%'", string.gsub, "alo", ".", "%x")
print("test8 passed")
