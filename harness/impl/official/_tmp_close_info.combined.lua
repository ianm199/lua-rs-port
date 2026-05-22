-- harness preamble: emulate the globals lua-c testes/all.lua sets
_soft = true
_port = true
_nomsg = true
_U = false
arg = arg or {}
_G = _G or _ENV
if _VERSION == nil then _VERSION = "Lua 5.4" end

local function inspect()
  for i = 0, 6 do
    local d = debug.getinfo(i, "Slntu")
    if not d then break end
    print(string.format("lvl=%d what=%s namewhat=%s name=%s linedefined=%d currentline=%d short_src=%s",
      i, tostring(d.what), tostring(d.namewhat), tostring(d.name),
      d.linedefined, d.currentline or -1, tostring(d.short_src)))
  end
end

local function func2close(f)
  return setmetatable({}, {__close = f})
end

local function foo(...)
  local x123 <close> = func2close(function()
    inspect()
    error("@x123")
  end)
end

local st, msg = xpcall(foo, debug.traceback)
print("===traceback===")
print(msg)
