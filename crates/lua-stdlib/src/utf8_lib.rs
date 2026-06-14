//! The `utf8` standard library: `char`, `codepoint`, `codes`, `len`, `offset`,
//! and `charpattern`. Present from Lua 5.3.
//!
//! GRADUATED (Phase-2 idiomatization). The line-by-line C correspondence to
//! `lutf8lib.c` has been removed; what guards this module now is the behavioral
//! net, in two layers:
//!   - the official 5.4 suite (`reference/lua-5.4.7-tests/utf8.lua`, run via the
//!     harness) and the cross-crate `multiversion_oracle`, and
//!   - `crates/lua-stdlib/tests/utf8_strengthen.rs`, which pins the **version
//!     seam** the 5.4-only suite cannot see: the decode/encode regime differs
//!     between 5.3 and 5.4+.
//!
//! The version seam is a single source of truth, [`DecodeMode`], resolved once
//! per call by [`decode_mode_for`]:
//!   - **5.3** — cap at `MAX_UNICODE`, at most a 4-byte sequence, surrogates
//!     ALWAYS accepted, the `lax` argument ignored (5.3's `utf8_decode` has no
//!     strict/lax parameter). `charpattern` stops the lead byte at `\xF4`.
//!   - **5.4+ strict** (default) — cap at `MAX_UTF` while decoding, then reject
//!     surrogates and values above `MAX_UNICODE`.
//!   - **5.4+ lax** — cap at `MAX_UTF`, accept surrogates. `charpattern` widens
//!     the lead byte to `\xFD`.
//!
//! LOAD-BEARING (do not reshape — only the validity ceiling is version-split):
//! the continuation-byte decode loop in [`utf8_decode`] and the backward-fill
//! encode in [`encode_utf8_codepoint`]. Those carry the bit-exact arithmetic and
//! are documented in place.

use crate::state_stub::{LuaState, LuaStateStubExt as _};
use lua_types::error::LuaError;
use lua_types::value::LuaValue;

const MAX_UNICODE: u32 = 0x10_FFFF;

const MAX_UTF: u32 = 0x7FFF_FFFF;

/// Integer wide enough for a decoded codepoint: 31 bits are needed for
/// `MAX_UTF`, so `u32` suffices on every Rust target.
type UtfInt = u32;

/// The 5.4+ `charpattern` (`lutf8lib.c` `UTF8PATT`): the lead-byte range runs to
/// `\xFD`, the ceiling for the extended (≤ `MAX_UTF`) range.
///
/// Embeds a NUL; length is `sizeof(UTF8PATT)/sizeof(char) - 1`.
const UTF8_PATT: &[u8] = b"[\x00-\x7F\xC2-\xFD][\x80-\xBF]*";

/// The 5.3 `charpattern`: the lead byte stops at `\xF4`, the ceiling for a
/// ≤ `MAX_UNICODE` (4-byte) sequence. 5.3's `utf8_decode` has no extended range,
/// so `lutf8lib.c` (5.3.6) ships this narrower pattern.
const UTF8_PATT_V53: &[u8] = b"[\x00-\x7F\xC2-\xF4][\x80-\xBF]*";

/// How one UTF-8 sequence is validated, derived once from the version + `lax`
/// flag (the C `lutf8lib.c` regime differs by family, not just by an argument):
///
/// - [`Self::V53`] — the 5.3 regime: cap at `MAX_UNICODE`, accept at most a
///   4-byte sequence, **never** reject surrogates, and ignore the `lax` argument
///   entirely (5.3's `utf8_decode` takes no strict/lax parameter).
/// - [`Self::Strict`] — 5.4+ default: cap at `MAX_UTF` while decoding, then
///   reject surrogates and values above `MAX_UNICODE`.
/// - [`Self::Lax`] — 5.4+ extended: cap at `MAX_UTF`, accept any well-formed
///   sequence including surrogates.
#[derive(Clone, Copy, PartialEq, Eq)]
enum DecodeMode {
    V53,
    Strict,
    Lax,
}

impl DecodeMode {
    /// The `(max continuation count, max codepoint)` pair the decode loop checks.
    ///
    /// 5.3 caps at a 4-byte / `MAX_UNICODE` sequence (no extended range); 5.4+
    /// allow a 6-byte / `MAX_UTF` sequence and apply the Unicode/surrogate
    /// ceiling separately via [`Self::rejects_surrogates`].
    fn length_and_value_ceiling(self) -> (usize, UtfInt) {
        match self {
            DecodeMode::V53 => (3, MAX_UNICODE),
            DecodeMode::Strict | DecodeMode::Lax => (5, MAX_UTF),
        }
    }

    /// Whether decoding rejects surrogates and values above `MAX_UNICODE`.
    ///
    /// Only the 5.4+ strict (default) regime does: 5.3 never rejects surrogates,
    /// and the 5.4+ lax regime disables the guard.
    fn rejects_surrogates(self) -> bool {
        self == DecodeMode::Strict
    }
}

/// Resolve the decode regime for `version` and the user's `lax` flag.
///
/// 5.3 is its own regime regardless of `lax`; 5.4+ pick strict-vs-lax from the
/// flag. This is the single source of truth for the version seam — the callers
/// never branch on the version for decoding.
fn decode_mode_for(version: lua_types::LuaVersion, lax: bool) -> DecodeMode {
    if version == lua_types::LuaVersion::V53 {
        DecodeMode::V53
    } else if lax {
        DecodeMode::Lax
    } else {
        DecodeMode::Strict
    }
}

// ── Internal helpers ───────────────────────────────────────────────────────

/// Translate a relative string position: negative values count backward from
/// the end, clamping to `0` when they reach before the start.
fn pos_relat(pos: i64, len: usize) -> i64 {
    if pos >= 0 {
        pos
    } else {
        let abs_pos = pos.unsigned_abs() as u64;
        if abs_pos > len as u64 {
            0
        } else {
            len as i64 + pos + 1
        }
    }
}

/// Return `true` if byte `c` is a UTF-8 continuation byte (`10xxxxxx`).
///
#[inline]
fn is_cont(c: u8) -> bool {
    (c & 0xC0) == 0x80
}

/// Return `true` if the byte at 0-based index `pos` in `s` is a continuation
/// byte, treating out-of-bounds (or negative) positions as non-continuation —
/// the bounds check stands in for the NUL terminator the upstream relies on.
#[inline]
fn is_cont_at(s: &[u8], pos: i64) -> bool {
    if pos < 0 {
        return false;
    }
    s.get(pos as usize).map_or(false, |&b| is_cont(b))
}

/// Decode one UTF-8 sequence from the start of `s`.
///
/// Returns `None` if the byte sequence is invalid, else
/// `Some((remaining_slice, codepoint))`.
///
/// `mode` selects the validity regime (see [`DecodeMode`]): the 5.3 path caps at
/// `MAX_UNICODE` and accepts surrogates; the 5.4+ strict/lax paths cap at
/// `MAX_UTF` and differ only on surrogate rejection.
///
/// LOAD-BEARING: the continuation-byte loop below is the faithful translation of
/// `lutf8lib.c`'s `utf8_decode`, including the order-sensitive details that look
/// odd in isolation — the high-bit walk `while c & 0x40` consumes one
/// continuation byte per iteration then shifts `c` left so the next leading bit
/// can be tested; `r` accumulates the low six bits of each continuation byte and
/// the leading byte's payload is folded in afterward via `(c & 0x7F) << count*5`;
/// `LIMITS[count]` is the smallest value legal for that sequence length (so a
/// too-small `r` is an overlong encoding), with `LIMITS[0] = u32::MAX` forcing an
/// error for a stray non-ASCII byte. Only the validity ceiling is version-split
/// (via [`DecodeMode`]); the arithmetic is shared and must not be reshaped.
fn utf8_decode(s: &[u8], mode: DecodeMode) -> Option<(&[u8], UtfInt)> {
    const LIMITS: [UtfInt; 6] = [u32::MAX, 0x80, 0x800, 0x10000, 0x200000, 0x4000000];

    if s.is_empty() {
        return None;
    }

    let mut c = s[0] as u32;
    let res: UtfInt;
    let advance: usize;

    if c < 0x80 {
        res = c;
        advance = 1;
    } else {
        let mut count: usize = 0;
        let mut r: UtfInt = 0;

        while c & 0x40 != 0 {
            count += 1;
            if count >= s.len() {
                return None;
            }
            let cc = s[count] as u32;

            if (cc & 0xC0) != 0x80 {
                return None;
            }

            r = (r << 6) | (cc & 0x3F);

            c <<= 1;
        }

        r |= (c & 0x7F) << (count as u32 * 5);

        let (max_count, max_value) = mode.length_and_value_ceiling();
        if count > max_count || r > max_value || r < LIMITS[count] {
            return None;
        }

        res = r;
        advance = count + 1;
        if advance > s.len() {
            return None;
        }
    }

    if mode.rejects_surrogates() && (res > MAX_UNICODE || (0xD800 <= res && res <= 0xDFFF)) {
        return None;
    }

    Some((&s[advance..], res))
}

/// Encode a codepoint (≤ `MAX_UTF`) as extended UTF-8 bytes, returned as a
/// `Vec<u8>` (the upstream fills a fixed buffer backwards).
///
/// LOAD-BEARING: `mfb` ("max value for a first byte") tracks the payload room in
/// the leading byte, halving each round as another continuation byte is emitted;
/// the continuation bytes are built low-to-high (reversed at the end) and the
/// leading byte's marker is `!mfb << 1`. The `wrapping_shl` is required: `!mfb`
/// overflows a plain `<< 1` in debug builds (e.g. `!0x1F = 0xFFFF_FFE0`,
/// `<< 1 = 0xFFFF_FFC0`, `as u8 = 0xC0`).
fn encode_utf8_codepoint(code: u32) -> Vec<u8> {
    debug_assert!(code <= MAX_UTF);

    if code < 0x80 {
        return vec![code as u8];
    }

    let mut x = code;
    let mut mfb: u32 = 0x3F;
    let mut bytes_rev: Vec<u8> = Vec::with_capacity(6);

    loop {
        bytes_rev.push(0x80 | (x & 0x3F) as u8);
        x >>= 6;
        mfb >>= 1;
        if x <= mfb {
            break;
        }
    }

    let leading = ((!mfb).wrapping_shl(1) as u8) | (x as u8);

    let mut result = Vec::with_capacity(bytes_rev.len() + 1);
    result.push(leading);
    for &b in bytes_rev.iter().rev() {
        result.push(b);
    }
    result
}

// ── Library functions ──────────────────────────────────────────────────────

/// `utf8.len(s [, i [, j [, lax]]])` → integer | (nil, integer)
///
/// Returns the number of UTF-8 characters that start in the byte range `[i,j]`
/// of string `s` (1-based, defaulting to the whole string).
/// On a malformed sequence, returns `(nil, position)` where `position` is the
/// 1-based byte offset of the first bad byte.
///
fn utf_len(state: &mut LuaState) -> Result<usize, LuaError> {
    // Clone to avoid holding a borrow across subsequent mutable state calls.
    let s: Vec<u8> = state.check_arg_string(1)?.to_vec();
    let len = s.len();

    let raw_posi: i64 = state.opt_arg_integer(2, 1)?;
    let mut posi: i64 = pos_relat(raw_posi, len);

    let raw_posj: i64 = state.opt_arg_integer(3, -1)?;
    let mut posj: i64 = pos_relat(raw_posj, len);

    let lax: bool = state.to_boolean(4);

    let version = state.global().lua_version;
    let mode = decode_mode_for(version, lax);
    let is_v53 = version == lua_types::LuaVersion::V53;
    let initial_msg: &[u8] = if is_v53 {
        b"initial position out of string"
    } else {
        b"initial position out of bounds"
    };
    let final_msg: &[u8] = if is_v53 {
        b"final position out of string"
    } else {
        b"final position out of bounds"
    };

    if posi < 1 {
        return Err(lua_vm::debug::arg_error_impl(state, 2, initial_msg));
    }
    posi -= 1;
    if posi > len as i64 {
        return Err(lua_vm::debug::arg_error_impl(state, 2, initial_msg));
    }

    posj -= 1;
    if posj >= len as i64 {
        return Err(lua_vm::debug::arg_error_impl(state, 3, final_msg));
    }

    let mut n: i64 = 0;

    while posi <= posj {
        match utf8_decode(&s[posi as usize..], mode) {
            None => {
                state.push(LuaValue::Nil);
                state.push(LuaValue::Int(posi + 1));
                return Ok(2);
            }
            Some((remaining, _)) => {
                posi = (len - remaining.len()) as i64;
                n += 1;
            }
        }
    }

    state.push(LuaValue::Int(n));
    Ok(1)
}

/// `utf8.codepoint(s [, i [, j [, lax]]])` → integer, ...
///
/// Returns the codepoints (as integers) for all characters starting in `s[i..j]`.
///
fn codepoint(state: &mut LuaState) -> Result<usize, LuaError> {
    let s: Vec<u8> = state.check_arg_string(1)?.to_vec();
    let len = s.len();

    let raw_posi: i64 = state.opt_arg_integer(2, 1)?;
    let posi: i64 = pos_relat(raw_posi, len);

    // Default for the end position is posi (1-based), giving a single character.
    let raw_pose: i64 = state.opt_arg_integer(3, posi)?;
    let pose: i64 = pos_relat(raw_pose, len);

    let lax: bool = state.to_boolean(4);

    let version = state.global().lua_version;
    let mode = decode_mode_for(version, lax);
    let bounds_msg: &[u8] = if version == lua_types::LuaVersion::V53 {
        b"out of range"
    } else {
        b"out of bounds"
    };
    if posi < 1 {
        return Err(lua_vm::debug::arg_error_impl(state, 2, bounds_msg));
    }

    if pose > len as i64 {
        return Err(lua_vm::debug::arg_error_impl(state, 3, bounds_msg));
    }

    if posi > pose {
        return Ok(0); // empty interval: no values
    }

    if pose - posi >= i32::MAX as i64 {
        return Err(LuaError::runtime(format_args!("string slice too long")));
    }

    let n_max = (pose - posi + 1) as i32;
    state.ensure_stack(n_max, "string slice too long")?;

    let mut pos: usize = (posi - 1) as usize;
    let end: usize = pose as usize;
    let mut count: usize = 0;

    while pos < end {
        match utf8_decode(&s[pos..], mode) {
            None => return Err(LuaError::runtime(format_args!("invalid UTF-8 code"))),
            Some((remaining, code)) => {
                state.push(LuaValue::Int(code as i64));
                count += 1;
                pos = len - remaining.len();
            }
        }
    }

    Ok(count)
}

/// Encode the codepoint at stack argument `arg` and return its UTF-8 bytes.
///
/// The accepted ceiling is version-split (the `utf8.char` `luaL_argcheck`):
/// 5.3 caps at `MAX_UNICODE`, 5.4+ at `MAX_UTF`. Returning the bytes lets
/// `utf_char` concatenate them without per-codepoint stack traffic.
fn get_utf_char_bytes(state: &mut LuaState, arg: i32) -> Result<Vec<u8>, LuaError> {
    let code = state.check_arg_integer(arg)? as u64;

    let max_code: u64 = if state.global().lua_version == lua_types::LuaVersion::V53 {
        MAX_UNICODE as u64
    } else {
        MAX_UTF as u64
    };
    if code > max_code {
        return crate::auxlib::arg_error(state, arg, b"value out of range").map(|_| Vec::new());
    }

    Ok(encode_utf8_codepoint(code as u32))
}

/// `utf8.char(n1, n2, ...)` → string
///
/// Returns the string formed by the UTF-8 encoding of the given codepoints.
///
fn utf_char(state: &mut LuaState) -> Result<usize, LuaError> {
    let n: i32 = state.stack_top() as i32;

    if n == 1 {
        let bytes = get_utf_char_bytes(state, 1)?;
        let s = state.intern_str(&bytes)?;
        state.push(LuaValue::Str(s));
    } else {
        let mut buf: Vec<u8> = Vec::new();
        for i in 1..=n {
            buf.extend_from_slice(&get_utf_char_bytes(state, i)?);
        }
        let s = state.intern_str(&buf)?;
        state.push(LuaValue::Str(s));
    }

    Ok(1)
}

/// `utf8.offset(s, n [, i])` → integer | nil
///
/// Returns the byte offset where the n-th character (counting from position `i`)
/// starts. Negative `n` counts from the end; `n == 0` returns the start of the
/// character that contains position `i`. Returns `nil` if the character cannot
/// be found.
///
/// `count` is `n` driven toward zero as characters are crossed. Each step does a
/// do-while-style inner walk: it moves one byte unconditionally, then skips over
/// any continuation bytes to land on the next leading byte (forward for `n > 0`,
/// backward for `n < 0`). Where C stops the inner walk on the NUL terminator,
/// the bounds check on `posi` does the same job here. Lua 5.5 additionally
/// returns the character's end byte (inclusive); 5.3/5.4 return only the start.
fn byte_offset(state: &mut LuaState) -> Result<usize, LuaError> {
    let s: Vec<u8> = state.check_arg_string(1)?.to_vec();
    let len = s.len();

    let n: i64 = state.check_arg_integer(2)?;

    let default_posi: i64 = if n >= 0 { 1 } else { len as i64 + 1 };

    let raw_posi: i64 = state.opt_arg_integer(3, default_posi)?;
    let posi_1based: i64 = pos_relat(raw_posi, len);

    let pos_msg: &[u8] = if state.global().lua_version == lua_types::LuaVersion::V53 {
        b"position out of range"
    } else {
        b"position out of bounds"
    };
    if posi_1based < 1 {
        return Err(lua_vm::debug::arg_error_impl(state, 3, pos_msg));
    }
    let mut posi: i64 = posi_1based - 1;
    if posi > len as i64 {
        return Err(lua_vm::debug::arg_error_impl(state, 3, pos_msg));
    }

    let mut count = n;

    if count == 0 {
        while posi > 0 && is_cont_at(&s, posi) {
            posi -= 1;
        }
    } else {
        if is_cont_at(&s, posi) {
            return Err(LuaError::runtime(format_args!(
                "initial position is a continuation byte"
            )));
        }

        if count < 0 {
            while count < 0 && posi > 0 {
                loop {
                    posi -= 1;
                    if posi == 0 || !is_cont_at(&s, posi) {
                        break;
                    }
                }
                count += 1;
            }
        } else {
            count -= 1;
            while count > 0 && posi < len as i64 {
                loop {
                    posi += 1;
                    if !is_cont_at(&s, posi) {
                        break;
                    }
                }
                count -= 1;
            }
        }
    }

    if count != 0 {
        state.push(LuaValue::Nil);
        return Ok(1);
    }

    state.push(LuaValue::Int(posi + 1));

    if state.global().lua_version != lua_types::LuaVersion::V55 {
        return Ok(1);
    }

    if s.get(posi as usize).is_some_and(|&b| b & 0x80 != 0) {
        if is_cont_at(&s, posi) {
            return Err(LuaError::runtime(format_args!(
                "initial position is a continuation byte"
            )));
        }
        while is_cont_at(&s, posi + 1) {
            posi += 1;
        }
    }
    state.push(LuaValue::Int(posi + 1));
    Ok(2)
}

/// Internal iterator body shared by `iter_aux_strict` and `iter_aux_lax`.
///
/// Stack on entry (from the generic for): (1) string, (2) current byte position
/// (0-based; initially pushed as 0 by `iter_codes`). Advances past any leading
/// continuation bytes, decodes the next character, and returns
/// `(next_1based_pos, codepoint)`, or nothing (0) when the string is exhausted.
/// A decode failure — or a decoded sequence immediately followed by a stray
/// continuation byte — raises "invalid UTF-8 code".
///
/// `strict` is the requested mode; the actual [`DecodeMode`] is resolved against
/// the version (5.3 ignores it, decoding in its own regime).
fn iter_aux(state: &mut LuaState, strict: bool) -> Result<usize, LuaError> {
    let s: Vec<u8> = state.check_arg_string(1)?.to_vec();
    let len = s.len();

    let mode = decode_mode_for(state.global().lua_version, !strict);

    let mut n: u64 = state.to_integer(2).unwrap_or(0) as u64;

    if (n as usize) < len {
        while (n as usize) < len && is_cont(s[n as usize]) {
            n += 1;
        }
    }

    if (n as usize) >= len {
        return Ok(0);
    }

    match utf8_decode(&s[n as usize..], mode) {
        None => Err(lua_vm::debug::c_api_runtime(
            state,
            b"invalid UTF-8 code".to_vec(),
        )),
        Some((remaining, code)) => {
            let next_pos = len - remaining.len();
            if next_pos < len && is_cont(s[next_pos]) {
                return Err(lua_vm::debug::c_api_runtime(
                    state,
                    b"invalid UTF-8 code".to_vec(),
                ));
            }
            state.push(LuaValue::Int((n + 1) as i64));
            state.push(LuaValue::Int(code as i64));
            Ok(2)
        }
    }
}

/// Strict iterator body: 5.4+ reject surrogates and values > `MAX_UNICODE`
/// (5.3 decodes in its own regime regardless).
fn iter_aux_strict(state: &mut LuaState) -> Result<usize, LuaError> {
    iter_aux(state, true)
}

/// Lax iterator body: 5.4+ accept extended UTF-8 up to `MAX_UTF`
/// (5.3 decodes in its own regime regardless).
fn iter_aux_lax(state: &mut LuaState) -> Result<usize, LuaError> {
    iter_aux(state, false)
}

/// `utf8.codes(s [, lax])` → function, string, integer
///
/// Returns the iterator triple `(f, s, 0)` for use in a generic for loop.
/// Each call to `f(s, pos)` returns the next `(pos, codepoint)` pair.
///
fn iter_codes(state: &mut LuaState) -> Result<usize, LuaError> {
    let lax: bool = state.to_boolean(2);

    let s: Vec<u8> = state.check_arg_string(1)?.to_vec();

    if s.first().map_or(false, |&b| is_cont(b)) {
        return Err(LuaError::arg_error(1, "invalid UTF-8 code"));
    }

    let iter_fn: fn(&mut LuaState) -> Result<usize, LuaError> =
        if lax { iter_aux_lax } else { iter_aux_strict };
    state.push_c_function(iter_fn)?;

    state.push_value_at(1)?;

    state.push(LuaValue::Int(0));

    Ok(3)
}

// ── Library registration ───────────────────────────────────────────────────

/// Function registration table for the `utf8` library.
///
/// "charpattern" is intentionally absent here; it is a string value and is
/// registered separately inside `open_utf8` via `lua_setfield`.
pub const FUNCS: &[(&[u8], fn(&mut LuaState) -> Result<usize, LuaError>)] = &[
    (b"offset", byte_offset),
    (b"codepoint", codepoint),
    (b"char", utf_char),
    (b"len", utf_len),
    (b"codes", iter_codes),
];

/// Open the `utf8` library.
///
/// Registers all functions from `FUNCS` into a new table, then sets
/// `utf8.charpattern` to the byte-string pattern matching one UTF-8 sequence.
/// The pattern's lead-byte ceiling is version-split: 5.3 stops at `\xF4`
/// (≤ `MAX_UNICODE`), 5.4+ extend it to `\xFD` (≤ `MAX_UTF`).
///
pub fn open_utf8(state: &mut LuaState) -> Result<usize, LuaError> {
    state.new_lib(FUNCS)?;

    let patt_bytes: &[u8] = if state.global().lua_version == lua_types::LuaVersion::V53 {
        UTF8_PATT_V53
    } else {
        UTF8_PATT
    };
    let patt = state.intern_str(patt_bytes)?;
    state.push(LuaValue::Str(patt));

    state.set_field(-2, b"charpattern")?;

    Ok(1)
}

// ──────────────────────────────────────────────────────────────────────────
// PORT STATUS
//   target_crate:  lua-stdlib
//   unsafe_blocks: 0
//   load-bearing:  the utf8_decode continuation-byte loop and the
//                  encode_utf8_codepoint backward-fill — bit-exact arithmetic,
//                  only the validity ceiling is version-split (DecodeMode).
//   net:           utf8.lua (official 5.4), multiversion_oracle, and the
//                  lua-stdlib utf8_strengthen tests (the 5.3-vs-5.4+ decode
//                  seam), plus check.sh 5.1-5.5. See GRADUATED.md "utf8".
// ──────────────────────────────────────────────────────────────────────────
