-- harness preamble: emulate the globals lua-c testes/all.lua sets
_soft = true
_port = true
_nomsg = true
_U = false
arg = arg or {}
_G = _G or _ENV
if _VERSION == nil then _VERSION = "Lua 5.4" end

-- Debug version of pm.lua - lines 195-300 of original to bisect failure
print('testing pattern matching')

local function checkerror (msg, f, ...)
  local s, err = pcall(f, ...)
  assert(not s and string.find(err, msg))
end


local function f (s, p)
  local i,e = string.find(s, p)
  if i then return string.sub(s, i, e) end
end

local function PU (p)
  p = string.gsub(p, "(" .. utf8.charpattern .. ")%?", function (c)
    return string.gsub(c, ".", "%0?")
  end)
  p = string.gsub(p, "%.", utf8.charpattern)
  return p
end
print('START')

-- pm.lua line 197+
assert(string.gsub("um (dois) tres (quatro)", "(%(%w+%))", string.upper) ==
            "um (DOIS) tres (QUATRO)")
print('LN197 ok')

do
  local function setglobal (n,v) rawset(_G, n, v) end
  string.gsub("a=roberto,roberto=a", "(%w+)=(%w%w*)", setglobal)
  assert(_G.a=="roberto" and _G.roberto=="a")
  _G.a = nil; _G.roberto = nil
end
print('LN200-205 ok')

function f(a,b) return string.gsub(a,'.',b) end
assert(string.gsub("trocar tudo em |teste|b| é |beleza|al|", "|([^|]*)|([^|]*)|", f) ==
            "trocar tudo em bbbbb é alalalalalal")
print('LN207-209 ok')

local function dostring (s) return load(s, "")() or "" end
assert(string.gsub("alo $a='x'$ novamente $return a$",
                   "$([^$]*)%$",
                   dostring) == "alo  novamente x")
print('LN211-214 ok')

local x = string.gsub("$x=string.gsub('alo', '.', string.upper)$ assim vai para $return x$",
         "$([^$]*)%$", dostring)
assert(x == ' assim vai para ALO')
_G.a, _G.x = nil
print('LN216-219 ok')

local t = {}
local s = 'a alo jose  joao'
local r = string.gsub(s, '()(%w+)()', function (a,w,b)
             assert(string.len(w) == b-a);
             t[a] = b-a;
           end)
assert(s == r and t[1] == 1 and t[3] == 3 and t[7] == 4 and t[13] == 4)
print('LN221-227 ok')

local function isbalanced (s)
  return not string.find(string.gsub(s, "%b()", ""), "[()]")
end

assert(isbalanced("(9 ((8))(\0) 7) \0\0 a b ()(c)() a"))
assert(not isbalanced("(9 ((8) 7) a b (\0 c) a"))
assert(string.gsub("alo 'oi' alo", "%b''", '"') == 'alo " alo')
print('LN234-236 ok')

local t = {"apple", "orange", "lime"; n=0}
assert(string.gsub("x and x and x", "x", function () t.n=t.n+1; return t[t.n] end)
        == "apple and orange and lime")
print('LN239-241 ok')

t = {n=0}
string.gsub("first second word", "%w%w*", function (w) t.n=t.n+1; t[t.n] = w end)
assert(t[1] == "first" and t[2] == "second" and t[3] == "word" and t.n == 3)
print('LN243-245 ok')

t = {n=0}
assert(string.gsub("first second word", "%w+",
         function (w) t.n=t.n+1; t[t.n] = w end, 2) == "first second word")
assert(t[1] == "first" and t[2] == "second" and t[3] == undef)
print('LN247-250 ok')

checkerror("invalid replacement value %(a table%)",
            string.gsub, "alo", ".", {a = {}})
checkerror("invalid capture index %%2", string.gsub, "alo", ".", "%2")
checkerror("invalid capture index %%0", string.gsub, "alo", "(%0)", "a")
checkerror("invalid capture index %%1", string.gsub, "alo", "(%1)", "a")
checkerror("invalid use of '%%'", string.gsub, "alo", ".", "%x")
print('LN252-257 ok')

local function rev (s)
  return string.gsub(s, "(.)(.+)", function (c,s1) return rev(s1)..c end)
end

local x = "abcdef"
assert(rev(rev(x)) == x)
print('LN273-278 ok')

assert(string.gsub("alo alo", ".", {}) == "alo alo")
assert(string.gsub("alo alo", "(.)", {a="AA", l=""}) == "AAo AAo")
assert(string.gsub("alo alo", "(.).", {a="AA", l="K"}) == "AAo AAo")
assert(string.gsub("alo alo", "((.)(.?))", {al="AA", o=false}) == "AAo AAo")
print('LN282-285 ok')

assert(string.gsub("alo alo", "().", {'x','yy','zzz'}) == "xyyzzz alo")
print('LN287 ok')

t = {}; setmetatable(t, {__index = function (t,s) return string.upper(s) end})
assert(string.gsub("a alo b hi", "%w%w+", t) == "a ALO b HI")
print('LN289-290 ok')

print('DONE')
