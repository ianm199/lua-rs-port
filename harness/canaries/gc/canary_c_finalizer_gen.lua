-- __gc metamethod fires when object becomes unreachable under gen mode.

local finalized = false
local t = setmetatable({}, {__gc = function() finalized = true end})
t = nil
collectgarbage("collect")  -- one full cycle to run finalizers
assert(finalized, "FAIL: __gc never fired")
print("PASS canary_c")
