-- harness preamble: emulate the globals lua-c testes/all.lua sets
_soft = true
_port = true
_nomsg = true
_U = false
arg = arg or {}
_G = _G or _ENV
if _VERSION == nil then _VERSION = "Lua 5.4" end

_soft = true

local function checkerror (msg, f, ...)
  local s, err = pcall(f, ...)
  assert(not s and string.find(err, msg))
end

print("Section 239-249:")
local t = {"apple", "orange", "lime"; n=0}
assert(string.gsub("x and x and x", "x", function () t.n=t.n+1; return t[t.n] end)
        == "apple and orange and lime")
print("239 ok")

t = {n=0}
string.gsub("first second word", "%w%w*", function (w) t.n=t.n+1; t[t.n] = w end)
assert(t[1] == "first" and t[2] == "second" and t[3] == "word" and t.n == 3)
print("244 ok")

t = {n=0}
assert(string.gsub("first second word", "%w+",
         function (w) t.n=t.n+1; t[t.n] = w end, 2) == "first second word")
assert(t[1] == "first" and t[2] == "second" and t[3] == undef)
print("250 ok")

print("Section 252-257 checkerror:")
checkerror("invalid replacement value %(a table%)",
            string.gsub, "alo", ".", {a = {}})
print("253 ok")
checkerror("invalid capture index %%2", string.gsub, "alo", ".", "%2")
print("254 ok")
checkerror("invalid capture index %%0", string.gsub, "alo", "(%0)", "a")
print("255 ok")
checkerror("invalid capture index %%1", string.gsub, "alo", "(%1)", "a")
print("256 ok")
checkerror("invalid use of '%%'", string.gsub, "alo", ".", "%x")
print("257 ok")

print("Section 273-278 rev:")
local function rev (s)
  return string.gsub(s, "(.)(.+)", function (c,s1) return rev(s1)..c end)
end
local x = "abcdef"
assert(rev(rev(x)) == x)
print("278 ok")

print("Section 282-290 gsub with tables:")
assert(string.gsub("alo alo", ".", {}) == "alo alo")
print("282 ok")
assert(string.gsub("alo alo", "(.)", {a="AA", l=""}) == "AAo AAo")
print("283 ok")
assert(string.gsub("alo alo", "(.).", {a="AA", l="K"}) == "AAo AAo")
print("284 ok")
assert(string.gsub("alo alo", "((.)(.?))", {al="AA", o=false}) == "AAo AAo")
print("285 ok")
assert(string.gsub("alo alo", "().", {'x','yy','zzz'}) == "xyyzzz alo")
print("287 ok")

t = {}; setmetatable(t, {__index = function (t,s) return string.upper(s) end})
assert(string.gsub("a alo b hi", "%w%w+", t) == "a ALO b HI")
print("290 ok")

print("Section 293-316 gmatch:")
local a = 0
for i in string.gmatch('abcde', '()') do assert(i == a+1); a=i end
assert(a==6)
print("296 ok")

t = {n=0}
for w in string.gmatch("first second word", "%w+") do
      t.n=t.n+1; t[t.n] = w
end
assert(t[1] == "first" and t[2] == "second" and t[3] == "word")
print("302 ok")

t = {3, 6, 9}
for i in string.gmatch ("xuxx uu ppar r", "()(.)%2") do
  assert(i == table.remove(t, 1))
end
assert(#t == 0)
print("308 ok")

t = {}
for i,j in string.gmatch("13 14 10 = 11, 15= 16, 22=23", "(%d+)%s*=%s*(%d+)") do
  t[tonumber(i)] = tonumber(j)
end
a = 0
for k,v in pairs(t) do assert(k+1 == v+0); a=a+1 end
assert(a == 3)
print("316 ok")
