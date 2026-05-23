--[[
string_ops_long.lua — same workload shape as string_ops.lua but scaled
~50x so the run lasts long enough to sample meaningfully with
/usr/bin/sample (which needs ~3+ seconds of wall to capture useful stacks).

Reference C Lua should complete in ~0.5s; lua-rs at 8x runs ~4s. That
gives the profiler enough wall to find the hot frames.

Deterministic: same checksum pattern as string_ops, scaled by the outer
multiplier so checksums multiply linearly.
]]

local pieces = {}
for i = 1, 1000 do
    pieces[#pieces + 1] = string.format("[item-%04d:%s]", i, "abcdefghij")
end
local big = table.concat(pieces)

local total_matches = 0
for _ = 1, 5000 do
    local count = 0
    for _ in string.gmatch(big, "item%-%d+") do
        count = count + 1
    end
    total_matches = total_matches + count
end

local upper_chars = 0
for _ = 1, 50 do
    local rewritten = string.gsub(big, "(%w+)", string.upper)
    for i = 1, #rewritten do
        local c = string.byte(rewritten, i)
        if c >= 65 and c <= 90 then upper_chars = upper_chars + 1 end
    end
end

assert(total_matches == 5000000,
       "string_ops_long match count mismatch: got " .. total_matches)
assert(upper_chars == 700000,
       "string_ops_long upper-char count mismatch: got " .. upper_chars)
io.write("string_ops_long.lua OK: matches=", total_matches,
         " upper=", upper_chars, "\n")
