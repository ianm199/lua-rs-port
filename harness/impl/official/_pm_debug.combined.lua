-- harness preamble: emulate the globals lua-c testes/all.lua sets
_soft = true
_port = true
_nomsg = true
_U = false
arg = arg or {}
_G = _G or _ENV
if _VERSION == nil then _VERSION = "Lua 5.4" end

-- Debug version of pm.lua with line markers
print('testing pattern matching')

local function checkerror (msg, f, ...)
  local s, err = pcall(f, ...)
  assert(not s and string.find(err, msg))
end


local function f (s, p)
  local i,e = string.find(s, p)
  if i then return string.sub(s, i, e) end
end

local a,b = string.find('', '')
assert(a == 1 and b == 0);
a,b = string.find('alo', '')
assert(a == 1 and b == 0)
a,b = string.find('a\0o a\0o a\0o', 'a', 1)
assert(a == 1 and b == 1)
a,b = string.find('a\0o a\0o a\0o', 'a\0o', 2)
assert(a == 5 and b == 7)
a,b = string.find('a\0o a\0o a\0o', 'a\0o', 9)
assert(a == 9 and b == 11)
a,b = string.find('a\0a\0a\0a\0\0ab', '\0ab', 2);
assert(a == 9 and b == 11);
a,b = string.find('a\0a\0a\0a\0\0ab', 'b')
assert(a == 11 and b == 11)
assert(not string.find('a\0a\0a\0a\0\0ab', 'b\0'))
assert(not string.find('', '\0'))
assert(string.find('alo123alo', '12') == 4)
assert(not string.find('alo123alo', '^12'))

assert(string.match("aaab", ".*b") == "aaab")
assert(string.match("aaa", ".*a") == "aaa")
assert(string.match("b", ".*b") == "b")

assert(string.match("aaab", ".+b") == "aaab")
assert(string.match("aaa", ".+a") == "aaa")
assert(not string.match("b", ".+b"))

assert(string.match("aaab", ".?b") == "ab")
assert(string.match("aaa", ".?a") == "aa")
assert(string.match("b", ".?b") == "b")

assert(f('aloALO', '%l*') == 'alo')
assert(f('aLo_ALO', '%a*') == 'aLo')

assert(f("  \n\r*&\n\r   xuxu  \n\n", "%g%g%g+") == "xuxu")

local function PU (p)
  p = string.gsub(p, "(" .. utf8.charpattern .. ")%?", function (c)
    return string.gsub(c, ".", "%0?")
  end)
  p = string.gsub(p, "%.", utf8.charpattern)
  return p
end


assert(f('aaab', 'a*') == 'aaa');
assert(f('aaa', '^.*$') == 'aaa');
assert(f('aaa', 'b*') == '');
assert(f('aaa', 'ab*a') == 'aa')
assert(f('aba', 'ab*a') == 'aba')
assert(f('aaab', 'a+') == 'aaa')
assert(f('aaa', '^.+$') == 'aaa')
assert(not f('aaa', 'b+'))
assert(not f('aaa', 'ab+a'))
assert(f('aba', 'ab+a') == 'aba')
assert(f('a$a', '.$') == 'a')
assert(f('a$a', '.%$') == 'a$')
assert(f('a$a', '.$.') == 'a$a')
assert(not f('a$a', '$$'))
assert(not f('a$b', 'a$'))
assert(f('a$a', '$') == '')
assert(f('', 'b*') == '')
assert(not f('aaa', 'bb*'))
assert(f('aaab', 'a-') == '')
assert(f('aaa', '^.-$') == 'aaa')
assert(f('aabaaabaaabaaaba', 'b.*b') == 'baaabaaabaaab')
assert(f('aabaaabaaabaaaba', 'b.-b') == 'baaab')
assert(f('alo xo', '.o$') == 'xo')
assert(f(' \n isto é assim', '%S%S*') == 'isto')
assert(f(' \n isto é assim', '%S*$') == 'assim')
assert(f(' \n isto é assim', '[a-z]*$') == 'assim')
assert(f('um caracter ? extra', '[^%sa-z]') == '?')
assert(f('', 'a?') == '')
assert(f('á', PU'á?') == 'á')
assert(f('ábl', PU'á?b?l?') == 'ábl')
assert(f('  ábl', PU'á?b?l?') == '')
assert(f('aa', '^aa?a?a') == 'aa')
assert(f(']]]áb', '[^]]+') == 'áb')
assert(f("0alo alo", "%x*") == "0a")
assert(f("alo alo", "%C+") == "alo alo")
print('+ MARK1')


local function f1 (s, p)
  p = string.gsub(p, "%%([0-9])", function (s)
        return "%" .. (tonumber(s)+1)
       end)
  p = string.gsub(p, "^(^?)", "%1()", 1)
  p = string.gsub(p, "($?)$", "()%1", 1)
  local t = {string.match(s, p)}
  return string.sub(s, t[1], t[#t] - 1)
end

assert(f1('alo alx 123 b\0o b\0o', '(..*) %1') == "b\0o b\0o")
assert(f1('axz123= 4= 4 34', '(.+)=(.*)=%2 %1') == '3= 4= 4 3')
assert(f1('=======', '^(=*)=%1$') == '=======')
assert(not string.match('==========', '^([=]*)=%1$'))

local function range (i, j)
  if i <= j then
    return i, range(i+1, j)
  end
end

local abc = string.char(range(0, 127)) .. string.char(range(128, 255));

assert(string.len(abc) == 256)

local function strset (p)
  local res = {s=''}
  string.gsub(abc, p, function (c) res.s = res.s .. c end)
  return res.s
end;

assert(string.len(strset('[\200-\210]')) == 11)

assert(strset('[a-z]') == "abcdefghijklmnopqrstuvwxyz")
assert(strset('[a-z%d]') == strset('[%da-uu-z]'))
assert(strset('[a-]') == "-a")
assert(strset('[^%W]') == strset('[%w]'))
assert(strset('[]%%]') == '%]')
assert(strset('[a%-z]') == '-az')
assert(strset('[%^%[%-a%]%-b]') == '-[]^ab')
assert(strset('%Z') == strset('[\1-\255]'))
assert(strset('.') == strset('[\1-\255%z]'))
print('+ MARK2');

assert(string.match("alo xyzK", "(%w+)K") == "xyz")
assert(string.match("254 K", "(%d*)K") == "")
assert(string.match("alo ", "(%w*)$") == "")
assert(not string.match("alo ", "(%w+)$"))
assert(string.find("(álo)", "%(á") == 1)
local a, b, c, d, e = string.match("âlo alo", PU"^(((.).). (%w*))$")
assert(a == 'âlo alo' and b == 'âl' and c == 'â' and d == 'alo' and e == nil)
a, b, c, d  = string.match('0123456789', '(.+(.?)())')
assert(a == '0123456789' and b == '' and c == 11 and d == nil)
print('+ MARK3')

assert(string.gsub('ülo ülo', 'ü', 'x') == 'xlo xlo')
assert(string.gsub('alo úlo  ', ' +$', '') == 'alo úlo')
assert(string.gsub('  alo alo  ', '^%s*(.-)%s*$', '%1') == 'alo alo')
assert(string.gsub('alo  alo  \n 123\n ', '%s+', ' ') == 'alo alo 123 ')
local t = "abç d"
a, b = string.gsub(t, PU'(.)', '%1@')
assert(a == "a@b@ç@ @d@" and b == 5)
a, b = string.gsub('abçd', PU'(.)', '%0@', 2)
assert(a == 'a@b@çd' and b == 2)
assert(string.gsub('alo alo', '()[al]', '%1') == '12o 56o')
assert(string.gsub("abc=xyz", "(%w*)(%p)(%w+)", "%3%2%1-%0") ==
              "xyz=abc-abc=xyz")
assert(string.gsub("abc", "%w", "%1%0") == "aabbcc")
assert(string.gsub("abc", "%w+", "%0%1") == "abcabc")
assert(string.gsub('áéí', '$', '\0óú') == 'áéí\0óú')
assert(string.gsub('', '^', 'r') == 'r')
assert(string.gsub('', '$', 'r') == 'r')
print('+ MARK4')


print('M4a: about to do empty matches block')
do
  assert(string.gsub("a b cd", " *", "-") == "-a-b-c-d-")
  print('M4b: passed gsub')
  local res = ""
  local sub = "a  \nbc\t\td"
  local i = 1
  for p, e in string.gmatch(sub, "()%s*()") do
    res = res .. string.sub(sub, i, p - 1) .. "-"
    i = e
  end
  print('M4c: gmatch result =', res)
  assert(res == "-a-b-c-d-")
end
print('M5')

assert(string.gsub("um (dois) tres (quatro)", "(%(%w+%))", string.upper) ==
            "um (DOIS) tres (QUATRO)")
print('M6')

do
  local function setglobal (n,v) rawset(_G, n, v) end
  string.gsub("a=roberto,roberto=a", "(%w+)=(%w%w*)", setglobal)
  assert(_G.a=="roberto" and _G.roberto=="a")
  _G.a = nil; _G.roberto = nil
end
print('M7')

function f(a,b) return string.gsub(a,'.',b) end
print('M7a: about to gsub with global f')
assert(string.gsub("trocar tudo em |teste|b| é |beleza|al|", "|([^|]*)|([^|]*)|", f) ==
            "trocar tudo em bbbbb é alalalalalal")
print('M8')

local function dostring (s) return load(s, "")() or "" end
print('M8a: about to gsub with dostring')
assert(string.gsub("alo $a='x'$ novamente $return a$",
                   "$([^$]*)%$",
                   dostring) == "alo  novamente x")
print('M9')

local x = string.gsub("$x=string.gsub('alo', '.', string.upper)$ assim vai para $return x$",
         "$([^$]*)%$", dostring)
assert(x == ' assim vai para ALO')
print('M10')
