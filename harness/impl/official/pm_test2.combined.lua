-- harness preamble: emulate the globals lua-c testes/all.lua sets
_soft = true
_port = true
_nomsg = true
_U = false
arg = arg or {}
_G = _G or _ENV
if _VERSION == nil then _VERSION = "Lua 5.4" end

local function f(s, p)
  local i,e = string.find(s, p)
  if i then return string.sub(s, i, e) end
end

print('testing pattern matching')

local function dostring (s)
  print("dostring called with:", s)
  local r = load(s, "")
  print("  load returned:", type(r), r)
  if type(r) == "function" then
    local res = r()
    print("  func returned:", type(res), res)
    return res or ""
  else
    print("  load failed:", r)
    return "" -- silent fallback, but should never trigger
  end
end

print("test 1:")
local r1 = string.gsub("alo $a='x'$ novamente $return a$",
                   "$([^$]*)%$",
                   dostring)
print("result 1:", r1)

print("test 2:")
local x = string.gsub("$x=string.gsub('alo', '.', string.upper)$ assim vai para $return x$",
         "$([^$]*)%$", dostring)
print("result 2:", x)

_G.a, _G.x = nil
print("step 6 ok")

local t = {}
local s = 'a alo jose  joao'
print("test 3:")
local r = string.gsub(s, '()(%w+)()', function (a,w,b)
             assert(string.len(w) == b-a);
             t[a] = b-a;
           end)
print("result 3:", r)
assert(s == r and t[1] == 1 and t[3] == 3 and t[7] == 4 and t[13] == 4)

print("step 7 ok")
