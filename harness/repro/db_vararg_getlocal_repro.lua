local function foo(a, ...)
  local t = table.pack(...)
  for i = 1, t.n do
    local n, v = debug.getlocal(1, -i)
    assert(n == "(vararg)")
    assert(v == t[i])
  end
  assert(not debug.getlocal(1, -(t.n + 1)))
  assert(not debug.setlocal(1, -(t.n + 1), 30))
  if t.n > 0 then
    (function(x)
      assert(debug.setlocal(2, -1, x) == "(vararg)")
      assert(debug.setlocal(2, -t.n, x) == "(vararg)")
    end)(430)
    assert(... == 430)
  end
end

foo()
foo(print)
foo(200, 3, 4)

local a = {}
for i = 1, 100 do a[i] = i end
foo(table.unpack(a))
