-- Exact, fast repro for reference/lua-5.4.7-tests/locals.lua's
-- "yielding inside closing metamethods after an error" block.

local function func2close(f)
  return setmetatable({}, { __close = f })
end

local co = coroutine.wrap(function()
  local function foo(err)
    local z <close> = func2close(function(_, msg)
      assert(msg == nil or msg == err + 20)
      coroutine.yield("z")
      return 100, 200
    end)

    local y <close> = func2close(function(_, msg)
      assert(msg == err or (msg == nil and err == 1))
      coroutine.yield("y")
      if err then error(err + 20) end
    end)

    local x <close> = func2close(function(_, msg)
      assert(msg == err or (msg == nil and err == 1))
      coroutine.yield("x")
      return 100, 200
    end)

    if err == 10 then error(err) else return 10, 20 end
  end

  coroutine.yield(pcall(foo, nil))
  coroutine.yield(pcall(foo, 1))
  return pcall(foo, 10)
end)

local function expect(label, expected, ...)
  local got = table.pack(...)
  print(label, table.unpack(got, 1, got.n))
  assert(got.n == #expected, label .. ": result count mismatch")
  for i = 1, #expected do
    assert(got[i] == expected[i],
      label .. ": result " .. i .. " expected " .. tostring(expected[i]) ..
      ", got " .. tostring(got[i]))
  end
end

expect("first-x", {"x"}, co())
expect("first-y", {"y"}, co())
expect("first-z", {"z"}, co())
expect("first-final", {true, 10, 20}, co())

expect("closeerr-x", {"x"}, co())
expect("closeerr-y", {"y"}, co())
expect("closeerr-z", {"z"}, co())
expect("closeerr-final", {false, 21}, co())

expect("bodyerr-x", {"x"}, co())
expect("bodyerr-y", {"y"}, co())
expect("bodyerr-z", {"z"}, co())
expect("bodyerr-final", {false, 30}, co())

local x = false
local y = false
local wrapped = coroutine.wrap(function()
  local xv <close> = func2close(function() x = true end)
  do
    local yv <close> = func2close(function() y = true end)
    coroutine.yield(100)
  end
  coroutine.yield(200)
  error(23)
end)

expect("wrapped-first", {100}, wrapped())
assert(not x and not y)
expect("wrapped-second", {200}, wrapped())
assert(not x and y)
local ok, msg = pcall(wrapped)
print("wrapped-final-state", tostring(ok), tostring(msg), tostring(x), tostring(y))
assert(not ok and msg == 23 and x and y)
