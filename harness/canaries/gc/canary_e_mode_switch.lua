-- Switching back and forth between modes must not corrupt heap or panic.

for i = 1, 5 do
  collectgarbage("generational")
  local t = {}
  for j = 1, 100 do t[j] = {j, j+1, j+2} end
  collectgarbage("step", 0)
  collectgarbage("incremental")
  collectgarbage("collect")
end
print("PASS canary_e")
