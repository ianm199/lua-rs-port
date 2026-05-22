-- harness preamble: emulate the globals lua-c testes/all.lua sets
_soft = true
_port = true
_nomsg = true
_U = false
arg = arg or {}
_G = _G or _ENV
if _VERSION == nil then _VERSION = "Lua 5.4" end

-- $Id: testes/literals.lua $
-- See Copyright Notice in file all.lua

print('testing scanner')

local debug = require "debug"


local function dostring (x) return assert(load(x), "")() end

dostring("x \v\f = \t\r 'a\0a' \v\f\f")
assert(x == 'a\0a' and string.len(x) == 3)
_G.x = nil

-- escape sequences
assert('\n\"\'\\' == [[

"'\]])

assert(string.find("\a\b\f\n\r\t\v", "^%c%c%c%c%c%c%c$"))

-- assume ASCII just for tests:
assert("\09912" == 'c12')
assert("\99ab" == 'cab')
assert("\099" == '\99')
assert("\099\n" == 'c\10')
assert('\0\0\0alo' == '\0' .. '\0\0' .. 'alo')

assert(010 .. 020 .. -030 == "1020-30")

-- hexadecimal escapes
assert("\x00\x05\x10\x1f\x3C\xfF\xe8" == "\0\5\16\31\60\255\232")

local function lexstring (x, y, n)
  local f = assert(load('return ' .. x ..
            ', require"debug".getinfo(1).currentline', ''))
  local s, l = f()
  assert(s == y and l == n)
end

lexstring("'abc\\z  \n   efg'", "abcefg", 2)
lexstring("'abc\\z  \n\n\n'", "abc", 4)
lexstring("'\\z  \n\t\f\v\n'",  "", 3)
lexstring("[[\nalo\nalo\n\n]]", "alo\nalo\n\n", 5)
lexstring("[[\nalo\ralo\n\n]]", "alo\nalo\n\n", 5)
lexstring("[[\nalo\ralo\r\n]]", "alo\nalo\n", 4)
lexstring("[[\ralo\n\ralo\r\n]]", "alo\nalo\n", 4)
lexstring("[[alo]\n]alo]]", "alo]\n]alo", 2)

assert("abc\z
        def\z
        ghi\z
       " == 'abcdefghi')


-- UTF-8 sequences
assert("\u{0}\u{00000000}\x00\0" == string.char(0, 0, 0, 0))

-- limits for 1-byte sequences
assert("\u{0}\u{7F}" == "\x00\x7F")

-- limits for 2-byte sequences
assert("\u{80}\u{7FF}" == "\xC2\x80\xDF\xBF")

-- limits for 3-byte sequences
assert("\u{800}\u{FFFF}" ==   "\xE0\xA0\x80\xEF\xBF\xBF")

-- limits for 4-byte sequences
assert("\u{10000}\u{1FFFFF}" == "\xF0\x90\x80\x80\xF7\xBF\xBF\xBF")

-- limits for 5-byte sequences
assert("\u{200000}\u{3FFFFFF}" == "\xF8\x88\x80\x80\x80\xFB\xBF\xBF\xBF\xBF")

-- limits for 6-byte sequences
assert("\u{4000000}\u{7FFFFFFF}" ==
       "\xFC\x84\x80\x80\x80\x80\xFD\xBF\xBF\xBF\xBF\xBF")


-- Error in escape sequences
local function lexerror (s, err)
  local st, msg = load('return ' .. s, '')
  if err ~= '<eof>' then err = err .. "'" end
  assert(not st and string.find(msg, "near .-" .. err))
end

lexerror([["abc\x"]], [[\x"]])
lexerror([["abc\x]], [[\x]])
lexerror([["\x]], [[\x]])
lexerror([["\x5"]], [[\x5"]])
lexerror([["\x5]], [[\x5]])
lexerror([["\xr"]], [[\xr]])
lexerror([["\xr]], [[\xr]])
lexerror([["\x.]], [[\x.]])
lexerror([["\x8%"]], [[\x8%%]])
lexerror([["\xAG]], [[\xAG]])
lexerror([["\g"]], [[\g]])
lexerror([["\g]], [[\g]])
lexerror([["\."]], [[\%.]])

lexerror([["\999"]], [[\999"]])
lexerror([["xyz\300"]], [[\300"]])
lexerror([["   \256"]], [[\256"]])

-- errors in UTF-8 sequences
lexerror([["abc\u{100000000}"]], [[abc\u{100000000]])   -- too large
lexerror([["abc\u11r"]], [[abc\u1]])    -- missing '{'
lexerror([["abc\u"]], [[abc\u"]])    -- missing '{'
lexerror([["abc\u{11r"]], [[abc\u{11r]])    -- missing '}'
lexerror([["abc\u{11"]], [[abc\u{11"]])    -- missing '}'
lexerror([["abc\u{11]], [[abc\u{11]])    -- missing '}'
lexerror([["abc\u{r"]], [[abc\u{r]])     -- no digits

-- unfinished strings
lexerror("[=[alo]]", "<eof>")
lexerror("[=[alo]=", "<eof>")
lexerror("[=[alo]", "<eof>")
lexerror("'alo", "<eof>")
lexerror("'alo \\z  \n\n", "<eof>")
lexerror("'alo \\z", "<eof>")
lexerror([['alo \98]], "<eof>")

-- valid characters in variable names
for i = 0, 255 do
  local s = string.char(i)
  assert(not string.find(s, "[a-zA-Z_]") == not load(s .. "=1", ""))
  assert(not string.find(s, "[a-zA-Z_0-9]") ==
         not load("a" .. s .. "1 = 1", ""))
end


-- long variable names

local var1 = string.rep('a', 15000) .. '1'
local var2 = string.rep('a', 15000) .. '2'
local prog = string.format([[
  %s = 5
  %s = %s + 1
  return function () return %s - %s end
]], var1, var2, var1, var1, var2)
local f = dostring(prog)
assert(_G[var1] == 5 and _G[var2] == 6 and f() == -1)
_G[var1], _G[var2] = nil
print('+')

-- escapes --
assert("\n\t" == [[

	]])
