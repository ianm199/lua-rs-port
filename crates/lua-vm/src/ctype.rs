//! Lua ctype — character-classification table and predicates.
//!
//! Ported from `reference/lua-5.4.7/src/lctype.c` and `lctype.h`.
//!
//! Lua ships its own ctype replacements, optimised for its specific needs.
//! These do **not** match the standard C `<ctype.h>` semantics exactly; in
//! particular `lislalpha` / `lislalnum` treat `'_'` as alphabetic, and the
//! table is seeded for ASCII byte ranges only (with high bytes left at 0x00
//! unless `LUA_UCID` is enabled — see PORT NOTE below).
//!
//! On ASCII targets (`LUA_USE_CTYPE=0`, the default) the implementation is a
//! 257-entry byte lookup table.  Each entry is a bitfield:
//!
//! | bit | name      | meaning                                      |
//! |-----|-----------|----------------------------------------------|
//! |  0  | ALPHABIT  | Lua-alphabetic: ASCII letters plus `_`       |
//! |  1  | DIGITBIT  | decimal digit `0`-`9`                        |
//! |  2  | PRINTBIT  | printable (graph + space)                    |
//! |  3  | SPACEBIT  | whitespace (ASCII space, TAB, LF, VT, FF, CR)|
//! |  4  | XDIGITBIT | hex digit `0`-`9`, `A`-`F`, `a`-`f`         |
//!
//! `test_prop(c, mask)` indexes the table as `CTYPE_TABLE[(c + 1) as usize]`,
//! which allows `c = -1` (the `EOZ` end-of-stream sentinel) without underflow.
//!
//! PORT NOTE: The C code supports a compile-time `LUA_UCID` flag that sets all
//! non-ASCII bytes (0x80-0xFF, minus invalid UTF-8 sequences) to `ALPHABIT`
//! so that Unicode identifiers are recognised.  That path (`NONA = 0x01`) is
//! not translated here; only the default `NONA = 0x00` path is ported.
//! Enable it in Phase B by introducing a Cargo feature flag.

const ALPHABIT: u32 = 0;

const DIGITBIT: u32 = 1;

const PRINTBIT: u32 = 2;

const SPACEBIT: u32 = 3;

const XDIGITBIT: u32 = 4;

// Inlined at each call site below as `1u8 << BIT`.

// LUA_UCID disabled — all non-ASCII bytes remain 0x00.

//
// UCHAR_MAX + 2 = 255 + 2 = 257 entries.
// Entry 0         → EOZ sentinel (c = -1; index = -1 + 1 = 0).
// Entries 1-256   → bytes 0x00-0xFF.
//
// Bit-flag legend (combined values seen in the table):
//   0x00 = no property (NUL, control chars, DEL, high bytes)
//   0x04 = PRINTBIT only (punctuation, symbols)
//   0x05 = ALPHABIT | PRINTBIT (non-hex letters + '_')
//   0x06 = DIGITBIT | PRINTBIT (this value does not appear alone; digits always have XDIGITBIT)
//   0x08 = SPACEBIT (TAB through CR)
//   0x0c = SPACEBIT | PRINTBIT (ASCII space 0x20)
//   0x15 = ALPHABIT | PRINTBIT | XDIGITBIT (A-F, a-f)
//   0x16 = DIGITBIT | PRINTBIT | XDIGITBIT (0-9)
pub(crate) static CTYPE_TABLE: [u8; 257] = [
    0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    //    BS    TAB   LF    VT    FF    CR
    0x00, 0x08, 0x08, 0x08, 0x08, 0x08, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    //    SPC   !     "     #     $     %     &     '
    0x0c, 0x04, 0x04, 0x04, 0x04, 0x04, 0x04, 0x04,
    //    (     )     *     +     ,     -     .     /
    0x04, 0x04, 0x04, 0x04, 0x04, 0x04, 0x04, 0x04,
    //    0     1     2     3     4     5     6     7
    0x16, 0x16, 0x16, 0x16, 0x16, 0x16, 0x16, 0x16,
    //    8     9     :     ;     <     =     >     ?
    0x16, 0x16, 0x04, 0x04, 0x04, 0x04, 0x04, 0x04,
    //    @     A     B     C     D     E     F     G
    0x04, 0x15, 0x15, 0x15, 0x15, 0x15, 0x15, 0x05,
    //    H     I     J     K     L     M     N     O
    0x05, 0x05, 0x05, 0x05, 0x05, 0x05, 0x05, 0x05,
    //    P     Q     R     S     T     U     V     W
    0x05, 0x05, 0x05, 0x05, 0x05, 0x05, 0x05, 0x05,
    //    X     Y     Z     [     \     ]     ^     _
    0x05, 0x05, 0x05, 0x04, 0x04, 0x04, 0x04, 0x05,
    //    `     a     b     c     d     e     f     g
    0x04, 0x15, 0x15, 0x15, 0x15, 0x15, 0x15, 0x05,
    //    h     i     j     k     l     m     n     o
    0x05, 0x05, 0x05, 0x05, 0x05, 0x05, 0x05, 0x05,
    //    p     q     r     s     t     u     v     w
    0x05, 0x05, 0x05, 0x05, 0x05, 0x05, 0x05, 0x05,
    //    x     y     z     {     |     }     ~     DEL
    0x05, 0x05, 0x05, 0x04, 0x04, 0x04, 0x04, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    //    0xC0 and 0xC1 are invalid UTF-8 leading bytes → 0x00
    //    0xC2-0xC7 are valid UTF-8 two-byte sequence starters → NONA (0x00 here)
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    //    0xF0-0xF4 are valid UTF-8 four-byte starters → NONA (0x00 here)
    //    0xF5-0xF7 are invalid UTF-8 → 0x00
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    //    all invalid UTF-8 sequences → 0x00
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
];

//
// `c` is an `i32` in Lua's internal representation: it is either a byte value
// 0-255, or -1 for EOZ.  Adding 1 shifts the range to 0-256, all valid indices
// into the 257-element table.
#[inline]
fn test_prop(c: i32, mask: u8) -> bool {
    debug_assert!(
        c >= -1 && c <= 255,
        "test_prop: c out of range: {}",
        c
    );
    CTYPE_TABLE[(c + 1) as usize] & mask != 0
}

//
// True for ASCII letters A-Z, a-z, and the underscore '_'.
// Includes non-ASCII bytes if LUA_UCID is enabled (not translated here).
#[inline]
pub(crate) fn lislalpha(c: i32) -> bool {
    test_prop(c, 1u8 << ALPHABIT)
}

//
// True for ASCII letters, digits, and '_'.
#[inline]
pub(crate) fn lislalnum(c: i32) -> bool {
    test_prop(c, (1u8 << ALPHABIT) | (1u8 << DIGITBIT))
}

//
// True for ASCII decimal digits '0'-'9'.
#[inline]
pub(crate) fn lisdigit(c: i32) -> bool {
    test_prop(c, 1u8 << DIGITBIT)
}

//
// True for ASCII whitespace: space (0x20), TAB (0x09), LF (0x0A),
// VT (0x0B), FF (0x0C), CR (0x0D).
#[inline]
pub(crate) fn lisspace(c: i32) -> bool {
    test_prop(c, 1u8 << SPACEBIT)
}

//
// True for printable characters: ASCII space through '~' (0x20-0x7E).
#[inline]
pub(crate) fn lisprint(c: i32) -> bool {
    test_prop(c, 1u8 << PRINTBIT)
}

//
// True for hexadecimal digits: '0'-'9', 'A'-'F', 'a'-'f'.
#[inline]
pub(crate) fn lisxdigit(c: i32) -> bool {
    test_prop(c, 1u8 << XDIGITBIT)
}

//      check_exp(('A' <= (c) && (c) <= 'Z') || (c) == ((c) | ('A' ^ 'a')), \
//                (c) | ('A' ^ 'a'))
//
// Converts an uppercase ASCII letter to its lowercase equivalent by setting
// bit 5 (0x20).  Only safe to call on uppercase letters A-Z, or on characters
// that already have bit 5 set (lowercase letters, '.', etc.).
//
// From macros.tsv: `check_exp(c, e)` → `{ debug_assert!(c); e }`.
// `'A' ^ 'a'` = 65 ^ 97 = 32 = 0x20.
#[inline]
pub(crate) fn ltolower(c: i32) -> i32 {
    debug_assert!(
        ('A' as i32 <= c && c <= 'Z' as i32) || c == (c | ('A' as i32 ^ 'a' as i32)),
        "ltolower: argument must be an uppercase letter or already lowercase/'.'"
    );
    c | ('A' as i32 ^ 'a' as i32)
}

// ──────────────────────────────────────────────────────────────────────────
// PORT STATUS
//   source:        src/lctype.c  (64 lines, 0 functions — only a table + header macros)
//   target_crate:  lua-vm
//   confidence:    high
//   todos:         0
//   port_notes:    1
//   unsafe_blocks: 0   (must be 0 outside explicit unsafe-budget crates)
//   notes:         Straightforward table + inline predicates; LUA_UCID path
//                  omitted (PORT NOTE in module doc). Phase B: add Cargo
//                  feature `lua-ucid` that substitutes NONA=0x01 for the
//                  non-ASCII rows.
// ──────────────────────────────────────────────────────────────────────────
