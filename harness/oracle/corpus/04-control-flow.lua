local function fact(n)
  if n <= 1 then return 1 end
  return n * fact(n - 1)
end

for i = 1, 5 do print(i, fact(i)) end

local s = 0
local i = 1
while i <= 10 do s = s + i; i = i + 1 end
print(s)
