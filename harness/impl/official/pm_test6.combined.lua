-- harness preamble: emulate the globals lua-c testes/all.lua sets
_soft = true
_port = true
_nomsg = true
_U = false
arg = arg or {}
_G = _G or _ENV
if _VERSION == nil then _VERSION = "Lua 5.4" end

_soft = true

print("test1: gmatch basic")
local a = 0
for i in string.gmatch('abcde', '()') do assert(i == a+1); a=i end
assert(a==6)
print("test1 passed")

print("test2: gmatch words")
local t = {n=0}
for w in string.gmatch("first second word", "%w+") do
      t.n=t.n+1; t[t.n] = w
end
assert(t[1] == "first" and t[2] == "second" and t[3] == "word")
print("test2 passed")

print("test3: gmatch with backref")
t = {3, 6, 9}
for i in string.gmatch ("xuxx uu ppar r", "()(.)%2") do
  assert(i == table.remove(t, 1))
end
assert(#t == 0)
print("test3 passed")

print("test4: gmatch dict")
t = {}
for i,j in string.gmatch("13 14 10 = 11, 15= 16, 22=23", "(%d+)%s*=%s*(%d+)") do
  t[tonumber(i)] = tonumber(j)
end
a = 0
for k,v in pairs(t) do assert(k+1 == v+0); a=a+1 end
assert(a == 3)
print("test4 passed")
