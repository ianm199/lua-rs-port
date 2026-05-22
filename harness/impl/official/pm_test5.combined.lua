-- harness preamble: emulate the globals lua-c testes/all.lua sets
_soft = true
_port = true
_nomsg = true
_U = false
arg = arg or {}
_G = _G or _ENV
if _VERSION == nil then _VERSION = "Lua 5.4" end

_soft = true

print("test1: rev")
local function rev (s)
  return string.gsub(s, "(.)(.+)", function (c,s1) return rev(s1)..c end)
end

local x = "abcdef"
assert(rev(rev(x)) == x)
print("test1 passed")

print("test2: gsub with tables")
assert(string.gsub("alo alo", ".", {}) == "alo alo")
print("test2.1 passed")
assert(string.gsub("alo alo", "(.)", {a="AA", l=""}) == "AAo AAo")
print("test2.2 passed")
assert(string.gsub("alo alo", "(.).", {a="AA", l="K"}) == "AAo AAo")
print("test2.3 passed")
assert(string.gsub("alo alo", "((.)(.?))", {al="AA", o=false}) == "AAo AAo")
print("test2.4 passed")
assert(string.gsub("alo alo", "().", {'x','yy','zzz'}) == "xyyzzz alo")
print("test2.5 passed")

print("test3: setmetatable __index")
local t = {}
setmetatable(t, {__index = function (t,s) return string.upper(s) end})
assert(string.gsub("a alo b hi", "%w%w+", t) == "a ALO b HI")
print("test3 passed")
