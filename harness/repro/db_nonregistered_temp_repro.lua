local function f()
  local name, value = debug.getlocal(2, 3)
  assert(name == "(temporary)" and value == 1)
  assert(not debug.getlocal(2, 4))
  debug.setlocal(2, 3, 10)
  return 20
end

local function g(a, b)
  return (a + 1) + f()
end

assert(g(0, 0) == 30)
