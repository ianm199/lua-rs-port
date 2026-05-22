local debug = require'debug'
local mt = {__index = function(a,b) return a+b end,
            __len = function(x) return math.floor(x) end}
print('before debug.setmetatable')
debug.setmetatable(10, mt)
print('after debug.setmetatable')
print(getmetatable(-2) == mt)
