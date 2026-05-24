local function getupvalues(f)
  local t = {}
  local i = 1
  while true do
    local name, value = debug.getupvalue(f, i)
    if not name then break end
    t[name] = value
    i = i + 1
  end
  return t
end

local a, b, c = 1, 2, 3
local function foo1(a) b = a; return c end
local function foo2(x) a = x; return c + b end

assert(not debug.getupvalue(foo1, 3))
assert(not debug.getupvalue(foo1, 0))
assert(not debug.setupvalue(foo1, 3, "xuxu"))

local t = getupvalues(foo1)
assert(t.a == nil and t.b == 2 and t.c == 3)

t = getupvalues(foo2)
assert(t.a == 1 and t.b == 2 and t.c == 3)

assert(debug.setupvalue(foo1, 1, "xuxu") == "b")
assert(({debug.getupvalue(foo2, 3)})[2] == "xuxu")
