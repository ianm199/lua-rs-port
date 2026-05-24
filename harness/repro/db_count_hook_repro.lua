local a = 0

debug.sethook(function() a = a + 1 end, "", 1)
a = 0
for _ = 1, 1000 do end
assert(1000 < a and a < 1012, a)

debug.sethook(function() a = a + 1 end, "", 4)
a = 0
for _ = 1, 1000 do end
assert(250 < a and a < 255, a)

local _, m, c = debug.gethook()
assert(m == "" and c == 4)

debug.sethook(function() a = a + 1 end, "", 4000)
a = 0
for _ = 1, 1000 do end
assert(a == 0, a)

debug.sethook()
