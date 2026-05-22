-- Weak-keyed and weak-valued tables under gen mode.

local wk = setmetatable({}, {__mode = "k"})
local wv = setmetatable({}, {__mode = "v"})

do
  local k = {}; local v = {}
  wk[k] = "kv"
  wv["kv"] = v
end

collectgarbage("collect")

local count_wk, count_wv = 0, 0
for _ in pairs(wk) do count_wk = count_wk + 1 end
for _ in pairs(wv) do count_wv = count_wv + 1 end
assert(count_wk == 0, "FAIL: weak-key entry survived: count=" .. count_wk)
assert(count_wv == 0, "FAIL: weak-value entry survived: count=" .. count_wv)
print("PASS canary_d")
