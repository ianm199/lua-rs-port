-- Minimal line-hook repro for reference/lua-c/testes/db.lua's large-gap loop.

local function trace_lines(src)
  local lines = {}
  local f = assert(load(src))
  local function hook(_, line)
    lines[#lines + 1] = line
  end
  debug.sethook(hook, "l"); f(); debug.sethook()
  return lines
end

local function same(a, b)
  if #a ~= #b then return false end
  for i = 1, #a do
    if a[i] ~= b[i] then return false end
  end
  return true
end

local function check(i, j)
  _G.a = nil
  local src = ([[
     local b = {10}
     a = b[1] %s + %s b[1]
     b = 4
  ]]):format(string.rep("\n", i), string.rep("\n", j))
  local expected = {1, 2 + i, 2 + i + j, 2 + i, 2 + i + j, 3 + i + j}
  local got = trace_lines(src)
  print(("gap i=%d j=%d"):format(i, j))
  print("got", table.concat(got, ","))
  print("expected", table.concat(expected, ","))
  assert(same(got, expected), "line trace mismatch")
end

check(1, 1)
check(128, 1)
check(1, 128)
