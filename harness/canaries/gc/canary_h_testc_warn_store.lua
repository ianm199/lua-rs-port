-- testC/ltests warning storage. Official gc.lua uses this to assert
-- warnings raised from __gc finalizer errors.

assert(T, "FAIL: testC table missing")
assert(_WARN == false, "FAIL: test warning sink did not initialize _WARN")

warn("@store")
warn("@direct", " warning")
assert(_WARN == "@direct warning", "FAIL: direct warning was not stored")
_WARN = false

local u = setmetatable({}, {__gc = function () error("@expected warning") end})
u = nil
collectgarbage()
assert(type(_WARN) == "string", "FAIL: finalizer warning was not stored")
assert(string.find(_WARN, "error in __gc", 1, true),
       "FAIL: stored warning did not name __gc")
assert(string.find(_WARN, "@expected warning", 1, true),
       "FAIL: stored warning did not include finalizer error")
_WARN = false
warn("@normal")

print("PASS canary_h")
