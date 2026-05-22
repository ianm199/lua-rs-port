-- harness preamble: emulate the globals lua-c testes/all.lua sets
_soft = true
_port = true
_nomsg = true
_U = false
arg = arg or {}
_G = _G or _ENV
if _VERSION == nil then _VERSION = "Lua 5.4" end

-- Simulate pm.lua up to line 219 area
local function f(s, p)
  local i,e = string.find(s, p)
  if i then return string.sub(s, i, e) end
end

print('testing pattern matching')
print('+')
print('+')
print('+')
print('+')

assert(string.gsub("um (dois) tres (quatro)", "(%(%w+%))", string.upper) ==
            "um (DOIS) tres (QUATRO)")
print("step 1 ok")

do
  local function setglobal (n,v) rawset(_G, n, v) end
  string.gsub("a=roberto,roberto=a", "(%w+)=(%w%w*)", setglobal)
  print("a =", _G.a, "roberto =", _G.roberto)
  assert(_G.a=="roberto" and _G.roberto=="a")
  _G.a = nil; _G.roberto = nil
end
print("step 2 ok")

function f(a,b) return string.gsub(a,'.',b) end
assert(string.gsub("trocar tudo em |teste|b| é |beleza|al|", "|([^|]*)|([^|]*)|", f) ==
            "trocar tudo em bbbbb é alalalalalal")
print("step 3 ok")

local function dostring (s) return load(s, "")() or "" end
assert(string.gsub("alo $a='x'$ novamente $return a$",
                   "$([^$]*)%$",
                   dostring) == "alo  novamente x")
print("step 4 ok")

local x = string.gsub("$x=string.gsub('alo', '.', string.upper)$ assim vai para $return x$",
         "$([^$]*)%$", dostring)
print("x =", x)
assert(x == ' assim vai para ALO')
print("step 5 ok")

_G.a, _G.x = nil
print("step 6 ok")
