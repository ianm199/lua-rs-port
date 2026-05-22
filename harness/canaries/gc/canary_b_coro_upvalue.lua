-- Open upvalue reachability through suspended coroutine.
-- A closure f captures a coroutine-local x; the coroutine is then dropped
-- (co = nil) but f remains. The collector must keep x reachable via f's
-- upvalue, which points into the coroutine's stack.

local f
local co = coroutine.create(function()
  local x = {sentinel = "initial"}
  f = function() return x end
  coroutine.yield()
  x = {sentinel = "after_resume"}     -- mutate via upvalue capture
  coroutine.yield()
end)

coroutine.resume(co)                    -- first yield; f now exists
assert(f().sentinel == "initial", "PRE-GC: f() pre-resume value lost")

coroutine.resume(co)                    -- second yield; x reassigned
assert(f().sentinel == "after_resume", "PRE-GC: f() post-resume value lost")

co = nil
collectgarbage("step", 0)
collectgarbage("step", 0)

assert(f, "POST-GC: f closure collected")
local ok, val = pcall(function() return f().sentinel end)
assert(ok, "POST-GC: f() raised error: " .. tostring(val))
assert(val == "after_resume", "POST-GC: f() returned " .. tostring(val))
print("PASS canary_b")
