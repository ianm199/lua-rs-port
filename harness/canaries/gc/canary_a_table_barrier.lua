-- Forward write barrier: an old table receives a new pointer.
-- C-Lua's gen mode tracks this via the touchedX list / barrier_back.

local t = {}
collectgarbage("collect")              -- promote t to old (no-op on incremental)
local new_val = {marker = "alive"}
t.ref = new_val
new_val = nil                          -- only t.ref keeps it alive
collectgarbage("step", 0)
collectgarbage("step", 0)
assert(t.ref ~= nil, "FAIL: t.ref collected")
assert(t.ref.marker == "alive", "FAIL: t.ref.marker missing")
print("PASS canary_a")
