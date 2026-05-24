local a = {}
local L = nil
local glob = 1
local oldglob = glob

debug.sethook(function(e, l)
  collectgarbage()
  local h, m, c = debug.gethook()
  assert(type(h) == "function" and m == "crl" and c == 0)
  if e == "line" then
    if glob ~= oldglob then
      L = l - 1
      oldglob = glob
    end
  elseif e == "call" then
    local f = debug.getinfo(2, "f").func
    a[f] = 1
    assert(type(f) == "function")
  else
    assert(e == "return")
  end
end, "crl")

local function foo()
  glob = glob + 1
end

foo()
