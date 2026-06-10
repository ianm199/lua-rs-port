-- Dead-key tombstone canary (C clearkey/equalkey parity; lgc.c).
--
-- A dynamic string key whose entry is nil'd must become collectible at the
-- next GC traversal WITHOUT leaving a dereferenceable key in the node:
-- probing the same bucket afterwards used to read freed memory
-- (use-after-free found by ASAN in db.lua, 2026-06-10). `next` iteration
-- across a tombstoned bucket must also stay sound.
local t = {}
do
  local k = "deadkey" .. tostring(12345)
  t[k] = 1
  t[k] = nil
end
collectgarbage()
collectgarbage()
local probes = 0
for i = 1, 50 do
  if t["probe" .. i] == nil then probes = probes + 1 end
end
t.live = 7
local seen = 0
for k, v in pairs(t) do seen = seen + 1 end
assert(probes == 50 and seen == 1 and t.live == 7)
print("PASS deadkey tombstone")
print("METRIC probes=50 pairs=1")
