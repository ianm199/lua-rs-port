local t = {10, 20, 30, name = "x", nested = {1, 2}}
print(t[1], t[2], t.name, t.nested[2])
for i, v in ipairs(t) do print(i, v) end
