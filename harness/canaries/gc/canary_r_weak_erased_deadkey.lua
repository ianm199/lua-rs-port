-- canary_r_weak_erased_deadkey — dead-key family, weak-table variant.
--
-- A weak table's hash node whose value was manually erased (a[k] = nil)
-- was SKIPPED by the weak prune pass, so its key was neither tombstoned
-- nor string-preserved. The key object got swept while the node kept a
-- dereferenceable ref to it; a later lookup that probed through the node
-- content-compared freed memory (TableInner::equal_key on a long string).
-- C parity: clearbykeys/clearbyvalues unconditionally clearkey() empty
-- entries (lgc.c). Distilled from gc.lua's "string keys in weak tables"
-- block, found by the rooting battery under LUA_RS_GC_QUARANTINE=1.

local a = setmetatable({}, {__mode = "kv"})
a[string.rep("a", 2^22)] = 25
a[string.rep("b", 2^22)] = {}
a[{}] = 14
collectgarbage()
local k, v = next(a)
assert(v == 25)
a[k] = nil
k = nil
collectgarbage()
assert(a[string.rep("b", 100)] == nil)
assert(next(a) == nil)
print("PASS canary_r_weak_erased_deadkey")
