-- harness preamble: emulate the globals lua-c testes/all.lua sets
_soft = true
_port = true
_nomsg = true
_U = false
arg = arg or {}
_G = _G or _ENV
if _VERSION == nil then _VERSION = "Lua 5.4" end

-- slice 3: through line 130 (bug since 5.4.0 - 257 constants)
;;print "testing syntax";;

local debug = require "debug"


local function checkload (s, msg)
  assert(string.find(select(2, load(s)), msg))
end

-- testing semicollons
local a
do ;;; end
; do ; a = 3; assert(a == 3) end;
;


-- invalid operations should not raise errors when not executed
if false then a = 3 // 0; a = 0 % 0 end


-- testing priorities

assert(2^3^2 == 2^(3^2));
assert(2^3*4 == (2^3)*4);
assert(2.0^-2 == 1/4 and -2^- -2 == - - -4);
assert(not nil and 2 and not(2>3 or 3<2));
assert(-3-1-5 == 0+0-9);
assert(-2^2 == -4 and (-2)^2 == 4 and 2*2-3-1 == 0);
assert(-3%5 == 2 and -3+5 == 2)
assert(2*1+3/3 == 3 and 1+2 .. 3*1 == "33");
assert(not(2+1 > 3*1) and "a".."b" > "a");

assert(0xF0 | 0xCC ~ 0xAA & 0xFD == 0xF4)
assert(0xFD & 0xAA ~ 0xCC | 0xF0 == 0xF4)
assert(0xF0 & 0x0F + 1 == 0x10)

assert(3^4//2^3//5 == 2)

assert(-3+4*5//2^3^2//9+4%10/3 == (-3)+(((4*5)//(2^(3^2)))//9)+((4%10)/3))

assert(not ((true or false) and nil))
assert(      true or false  and nil)

-- old bug
assert((((1 or false) and true) or false) == true)
assert((((nil and true) or false) and true) == false)

local a,b = 1,nil;
assert(-(1 or 2) == -1 and (1 and 2)+(-1.25 or -4) == 0.75);
local x = ((b or a)+1 == 2 and (10 or a)+1 == 11); assert(x);
x = (((2<3) or 1) == true and (2<3 and 4) == 4); assert(x);

local x, y = 1, 2;
assert((x>y) and x or y == 2);
x,y=2,1;
assert((x>y) and x or y == 2);

assert(1234567890 == tonumber('1234567890') and 1234567890+1 == 1234567891)

do   -- testing operators with diffent kinds of constants
  local operand = {3, 100, 5.0, -10, -5.0, 10000, -10000}
  local operator = {"+", "-", "*", "/", "//", "%", "^",
                    "&", "|", "^", "<<", ">>",
                    "==", "~=", "<", ">", "<=", ">=",}
  for _, op in ipairs(operator) do
    local f = assert(load(string.format([[return function (x,y)
                return x %s y
              end]], op)))();
    for _, o1 in ipairs(operand) do
      for _, o2 in ipairs(operand) do
        local gab = f(o1, o2)

        _ENV.XX = o1
        local code = string.format("return XX %s %s", op, o2)
        local res = assert(load(code))()
        assert(res == gab)

        _ENV.XX = o2
        code = string.format("return (%s) %s XX", o1, op)
        res = assert(load(code))()
        assert(res == gab)

        code = string.format("return (%s) %s %s", o1, op, o2)
        res = assert(load(code))()
        assert(res == gab)
      end
    end
  end
  _ENV.XX = nil
end


-- silly loops
repeat until 1; repeat until true;
while false do end; while nil do end;
print("checkpoint A: after silly loops")

do  -- test old bug (first name could not be an `upvalue')
 local a; local function f(x) x={a=1}; x={x=1}; x={G=1} end
end
print("checkpoint B: after upval old bug")

do   -- bug since 5.4.0
  -- create code with a table using more than 256 constants
  local code = {"local x = {"}
  for i = 1, 257 do
    code[#code + 1] = i .. ".1,"
  end
  code[#code + 1] = "};"
  code = table.concat(code)

  -- add "ret" to the end of that code and checks that
  -- it produces the expected value "val"
  local function check (ret, val)
    local code = code .. ret
    code = load(code)
    assert(code() == val)
  end

  check("return (1 ~ (2 or 3))", 1 ~ 2)
  check("return (1 | (2 or 3))", 1 | 2)
  check("return (1 + (2 or 3))", 1 + 2)
  check("return (1 << (2 or 3))", 1 << 2)
end
print("checkpoint C: after 256-constants bug")
