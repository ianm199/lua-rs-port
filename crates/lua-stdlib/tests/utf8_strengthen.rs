//! Reference-pinned behavioral net for the `utf8` library's version seams.
//!
//! `utf8` exists only from Lua 5.3, and its decode/encode regime changed
//! materially between 5.3 and 5.4: 5.4 introduced the *extended* (lax) range up
//! to `MAXUTF` (`0x7FFFFFFF`) with strict-mode surrogate rejection, while 5.3's
//! `utf8_decode` (`lutf8lib.c`, 5.3.6) is a strictly simpler function — it caps
//! unconditionally at `MAXUNICODE` (`0x10FFFF`), accepts at most a 4-byte
//! sequence (`count > 3`), **accepts surrogates**, and has **no strict/lax
//! distinction at all** (the 4th `lax` argument does not even reach `decode`).
//!
//! Those differences are invisible to the official 5.4 `utf8.lua` suite (which
//! only runs against 5.4 semantics) and were thinly netted by the cross-crate
//! oracle, so this file pins them per version against the reference binaries
//! (`/tmp/lua-refs/bin/lua5.x`, see `specs/oracle/CONTRACT.md`). Every expected
//! constant below is the literal output of the named reference binary on the
//! quoted snippet; nothing is pinned to the impl's own output.
//!
//! `omnilua` is a dev-dependency here (it depends on `lua-stdlib`, so a normal
//! dependency would cycle — see `Cargo.toml`).

use omnilua::{Lua, LuaVersion, Value};

/// The three versions that ship the `utf8` library (added in 5.3).
const UTF8_VERSIONS: [LuaVersion; 3] = [LuaVersion::V53, LuaVersion::V54, LuaVersion::V55];

/// Evaluate `code` under `version`, returning the single result coerced to `T`.
fn eval<T: omnilua::FromLuaMulti>(version: LuaVersion, code: &str) -> T {
    let lua = Lua::new_versioned(version);
    lua.load(code)
        .eval()
        .unwrap_or_else(|e| panic!("eval of `{code}` failed under {version:?}: {e:?}"))
}

/// Evaluate `code` and return its single string result as raw bytes.
///
/// Used for hex dumps and pinned error messages — `utf8` deals in byte strings,
/// so the result is compared as `&[u8]`, never as text.
fn eval_bytes(version: LuaVersion, code: &str) -> Vec<u8> {
    match eval::<Value>(version, code) {
        Value::String(s) => s.as_bytes().expect("string bytes"),
        other => panic!("`{code}` under {version:?} returned {other:?}, expected a string"),
    }
}

/// `pcall(fn, args...)`-shaped probe: returns `"ok:<v>"` or `"err:<msg>"` bytes.
///
/// The call is made **directly** as `pcall(fn, args)` (NOT wrapped in an
/// anonymous function). That distinction is load-bearing for error-wording:
/// Lua recovers the field name `utf8.char` only for a direct field call; a
/// `pcall(function() return utf8.char(...) end)` wrapper recovers the bare
/// `char` and adds a chunk-location prefix (verified identical in the impl and
/// the reference binary). The success branch stringifies the first result.
fn probe_call(version: LuaVersion, callee: &str, args: &str) -> Vec<u8> {
    let code = format!(
        "local ok, r = pcall({callee}, {args})\n\
         if ok then return 'ok:' .. tostring(r) else return 'err:' .. tostring(r) end"
    );
    eval_bytes(version, &code)
}

// ── charpattern: the lead-byte ceiling is version-split ─────────────────────
//
// 5.3.6 `UTF8PATT` is `[\0-\x7F\xC2-\xF4][\x80-\xBF]*` (max lead `\xF4`, the
// ceiling for a ≤U+10FFFF sequence); 5.4.7/5.5.0 widened it to `\xC2-\xFD` for
// the extended range. Hex captured via
//   lua5.x -e 'local p=utf8.charpattern ... string.format("%02X",p:byte(i))'

#[test]
fn charpattern_lead_byte_ceiling_is_version_split() {
    let dump = "local p = utf8.charpattern; local o = {}; \
                for i = 1, #p do o[i] = string.format('%02X', p:byte(i)) end; \
                return table.concat(o)";

    // 5.3.6 reference: lead-byte range ends at F4.
    assert_eq!(
        eval_bytes(LuaVersion::V53, dump),
        b"5B002D7FC22DF45D5B802DBF5D2A".to_vec(),
        "5.3 charpattern must cap the lead byte at \\xF4 (lutf8lib.c 5.3.6 UTF8PATT)"
    );

    // 5.4.7 / 5.5.0 reference: lead-byte range ends at FD (extended range).
    for v in [LuaVersion::V54, LuaVersion::V55] {
        assert_eq!(
            eval_bytes(v, dump),
            b"5B002D7FC22DFD5D5B802DBF5D2A".to_vec(),
            "5.4+ charpattern must cap the lead byte at \\xFD (extended UTF8PATT)"
        );
    }
}

// ── codepoint: the lax upper bound is version-split ─────────────────────────
//
// `utf8.codepoint("\xF4\x90\x80\x80", 1, 1, true)` decodes 0x110000 in lax mode.
// 5.3.6 has NO lax range — it caps at MAXUNICODE and rejects → "invalid UTF-8
// code"; 5.4.7/5.5.0 accept it (extended range) → 1114112.

#[test]
fn codepoint_lax_bound_is_version_split() {
    let (callee, args) = ("utf8.codepoint", "'\\xF4\\x90\\x80\\x80', 1, 1, true");

    assert_eq!(
        probe_call(LuaVersion::V53, callee, args),
        b"err:invalid UTF-8 code".to_vec(),
        "5.3 has no extended range: lax 0x110000 must be rejected"
    );
    for v in [LuaVersion::V54, LuaVersion::V55] {
        assert_eq!(
            probe_call(v, callee, args),
            b"ok:1114112".to_vec(),
            "5.4+ lax must accept the extended codepoint 0x110000"
        );
    }
}

#[test]
fn codepoint_lax_accepts_maxutf_only_from_54() {
    // 0x7FFFFFFF encoded (6 bytes). 5.3 rejects (only 4-byte / ≤MAXUNICODE);
    // 5.4+ accept → 2147483647.
    let (callee, args) = ("utf8.codepoint", "'\\xFD\\xBF\\xBF\\xBF\\xBF\\xBF', 1, 1, true");

    assert_eq!(
        probe_call(LuaVersion::V53, callee, args),
        b"err:invalid UTF-8 code".to_vec(),
        "5.3 must reject a 6-byte / 0x7FFFFFFF sequence even in lax"
    );
    for v in [LuaVersion::V54, LuaVersion::V55] {
        assert_eq!(
            probe_call(v, callee, args),
            b"ok:2147483647".to_vec(),
            "5.4+ lax must accept MAXUTF (0x7FFFFFFF)"
        );
    }
}

// ── surrogates: 5.3 accepts them unconditionally; 5.4+ reject in strict ─────
//
// 5.3.6 `utf8_decode` has no D800..DFFF guard, so a surrogate decodes fine in
// BOTH default and lax mode. 5.4 added the strict guard: default (strict)
// rejects, lax accepts.

#[test]
fn surrogate_strict_default_is_version_split() {
    // Default mode (no 4th arg → strict on 5.4+).
    let (callee, args) = ("utf8.codepoint", "'\\u{D800}'");

    assert_eq!(
        probe_call(LuaVersion::V53, callee, args),
        b"ok:55296".to_vec(),
        "5.3 has no surrogate guard: default decode of U+D800 must succeed"
    );
    for v in [LuaVersion::V54, LuaVersion::V55] {
        assert_eq!(
            probe_call(v, callee, args),
            b"err:invalid UTF-8 code".to_vec(),
            "5.4+ strict (default) must reject the surrogate U+D800"
        );
    }
}

#[test]
fn surrogate_lax_accepted_on_all_versions() {
    // Lax mode (4th arg true) accepts the surrogate on every version that has
    // utf8 — on 5.3 the arg is a no-op, on 5.4+ lax disables the strict guard.
    let (callee, args) = ("utf8.codepoint", "'\\u{D800}', 1, 1, true");
    for v in UTF8_VERSIONS {
        assert_eq!(
            probe_call(v, callee, args),
            b"ok:55296".to_vec(),
            "lax decode of U+D800 must succeed on every utf8 version"
        );
    }
}

#[test]
fn len_over_surrogate_is_version_split() {
    // `utf8.len("\u{D800}")` — 5.3 counts the surrogate (1); 5.4+ strict report
    // the malformed position (nil, 1).
    assert_eq!(
        eval::<i64>(LuaVersion::V53, "return utf8.len('\\u{D800}')"),
        1,
        "5.3 len must count the surrogate as one character"
    );
    for v in [LuaVersion::V54, LuaVersion::V55] {
        let (a, b): (Value, i64) =
            eval(v, "local a, b = utf8.len('\\u{D800}'); return a, b");
        assert!(
            matches!(a, Value::Nil),
            "5.4+ len must fail (nil) on the surrogate under {v:?}, got {a:?}"
        );
        assert_eq!(b, 1, "5.4+ len failure position must be 1 under {v:?}");
    }
}

#[test]
fn codes_over_surrogate_is_version_split() {
    // Iterating a surrogate: 5.3 yields the codepoint; 5.4+ raise the runtime
    // error "invalid UTF-8 code". That error carries a chunk-location prefix
    // (the official suite matches it with `string.find`, not equality), so this
    // probe reports whether the iteration succeeded and substring-matches the
    // message — mirroring `checkerror("invalid UTF%-8 code", ...)`.
    let code = "local ok, r = pcall(function() \
                  local c; for _, cc in utf8.codes('\\u{D800}') do c = cc end; return c \
                end)\n\
                if ok then return 'ok:' .. tostring(r) \
                elseif string.find(r, 'invalid UTF%-8 code') then return 'err:invalid UTF-8 code' \
                else return 'err:' .. tostring(r) end";

    assert_eq!(
        eval_bytes(LuaVersion::V53, code),
        b"ok:55296".to_vec(),
        "5.3 codes() must iterate the surrogate codepoint"
    );
    for v in [LuaVersion::V54, LuaVersion::V55] {
        assert_eq!(
            eval_bytes(v, code),
            b"err:invalid UTF-8 code".to_vec(),
            "5.4+ codes() must reject the surrogate"
        );
    }
}

// ── char: the encode ceiling is version-split ───────────────────────────────
//
// 5.3.6 `utf8.char` checks `code <= MAXUNICODE`; 5.4.7/5.5.0 check
// `code <= MAXUTF`. Error wording is byte-identical across versions:
// "bad argument #1 to 'utf8.char' (value out of range)".

#[test]
fn char_encode_ceiling_is_version_split() {
    // 0x110000 is above MAXUNICODE but below MAXUTF.
    assert_eq!(
        probe_call(LuaVersion::V53, "utf8.char", "0x110000"),
        b"err:bad argument #1 to 'utf8.char' (value out of range)".to_vec(),
        "5.3 char must reject 0x110000 (> MAXUNICODE)"
    );
    for v in [LuaVersion::V54, LuaVersion::V55] {
        // 5.4+ encode 0x110000 into a 4-byte sequence.
        assert_eq!(
            eval::<i64>(v, "return #utf8.char(0x110000)"),
            4,
            "5.4+ char must encode 0x110000 as 4 bytes under {v:?}"
        );
    }
}

#[test]
fn char_rejects_above_maxutf_on_all_versions() {
    // 0x80000000 is above MAXUTF on every version → same error.
    for v in UTF8_VERSIONS {
        assert_eq!(
            probe_call(v, "utf8.char", "0x80000000"),
            b"err:bad argument #1 to 'utf8.char' (value out of range)".to_vec(),
            "char must reject 0x80000000 on every utf8 version under {v:?}"
        );
    }
}

#[test]
fn char_negative_is_rejected_on_all_versions() {
    for v in UTF8_VERSIONS {
        assert_eq!(
            probe_call(v, "utf8.char", "-1"),
            b"err:bad argument #1 to 'utf8.char' (value out of range)".to_vec(),
            "char must reject a negative codepoint under {v:?}"
        );
    }
}

// ── offset: edges the official suite under-pins ─────────────────────────────
//
// `héllo` is `h é(C3 A9) l l o` — 6 bytes, 5 characters; the `é` occupies
// bytes 2..3, so byte position 3 is mid-character.

#[test]
fn offset_zero_finds_char_start() {
    // n == 0 from a mid-character byte returns the start of that character.
    for v in UTF8_VERSIONS {
        assert_eq!(
            eval::<i64>(v, "return utf8.offset('héllo', 0, 3)"),
            2,
            "offset(s, 0, 3) must land on the start of 'é' (byte 2) under {v:?}"
        );
    }
}

#[test]
fn offset_negative_n_counts_back_from_end() {
    for v in UTF8_VERSIONS {
        // -1 from the implicit end (#s+1 = 7) → start of the last char 'o' (6).
        assert_eq!(
            eval::<i64>(v, "return utf8.offset('héllo', -1)"),
            6,
            "offset(s, -1) must find the last character start under {v:?}"
        );
        // -2 → the char before that ('l' at byte 5).
        assert_eq!(
            eval::<i64>(v, "return utf8.offset('héllo', -2)"),
            5,
            "offset(s, -2) must skip back two characters under {v:?}"
        );
    }
}

#[test]
fn offset_past_end_returns_nil() {
    for v in UTF8_VERSIONS {
        assert!(
            matches!(
                eval::<Value>(v, "return utf8.offset('abc', 5)"),
                Value::Nil
            ),
            "offset past the end must return nil under {v:?}"
        );
    }
}

#[test]
fn offset_initial_position_out_of_bounds_wording_is_version_split() {
    // 5.3 says "out of range"; 5.4+ say "out of bounds".
    assert_eq!(
        probe_call(LuaVersion::V53, "utf8.offset", "'abc', 1, 5"),
        b"err:bad argument #3 to 'utf8.offset' (position out of range)".to_vec(),
        "5.3 offset OOB wording"
    );
    for v in [LuaVersion::V54, LuaVersion::V55] {
        assert_eq!(
            probe_call(v, "utf8.offset", "'abc', 1, 5"),
            b"err:bad argument #3 to 'utf8.offset' (position out of bounds)".to_vec(),
            "5.4+ offset OOB wording under {v:?}"
        );
    }
}

#[test]
fn offset_into_continuation_byte_errors() {
    // `é` = C3 A9; position 2 is its continuation byte. Wording is identical on
    // every version: "initial position is a continuation byte".
    for v in UTF8_VERSIONS {
        assert_eq!(
            probe_call(v, "utf8.offset", "'é', 1, 2"),
            b"err:initial position is a continuation byte".to_vec(),
            "offset into a continuation byte must error under {v:?}"
        );
    }
}

// ── len / codepoint: bounds-error wording is version-split ──────────────────

#[test]
fn len_out_of_bounds_wording_is_version_split() {
    // initial position (arg #2) too small.
    assert_eq!(
        probe_call(LuaVersion::V53, "utf8.len", "'abc', 0, 2"),
        b"err:bad argument #2 to 'utf8.len' (initial position out of string)".to_vec(),
        "5.3 len initial-position wording"
    );
    // final position (arg #3) too large.
    assert_eq!(
        probe_call(LuaVersion::V53, "utf8.len", "'abc', 1, 4"),
        b"err:bad argument #3 to 'utf8.len' (final position out of string)".to_vec(),
        "5.3 len final-position wording"
    );
    for v in [LuaVersion::V54, LuaVersion::V55] {
        assert_eq!(
            probe_call(v, "utf8.len", "'abc', 0, 2"),
            b"err:bad argument #2 to 'utf8.len' (initial position out of bounds)".to_vec(),
            "5.4+ len initial-position wording under {v:?}"
        );
        assert_eq!(
            probe_call(v, "utf8.len", "'abc', 1, 4"),
            b"err:bad argument #3 to 'utf8.len' (final position out of bounds)".to_vec(),
            "5.4+ len final-position wording under {v:?}"
        );
    }
}

#[test]
fn codepoint_out_of_bounds_wording_is_version_split() {
    assert_eq!(
        probe_call(LuaVersion::V53, "utf8.codepoint", "'abc', 5"),
        b"err:bad argument #3 to 'utf8.codepoint' (out of range)".to_vec(),
        "5.3 codepoint OOB wording"
    );
    for v in [LuaVersion::V54, LuaVersion::V55] {
        assert_eq!(
            probe_call(v, "utf8.codepoint", "'abc', 5"),
            b"err:bad argument #3 to 'utf8.codepoint' (out of bounds)".to_vec(),
            "5.4+ codepoint OOB wording under {v:?}"
        );
    }
}

// ── malformed-continuation len reporting (the position of the first bad byte) ─

#[test]
fn len_reports_first_malformed_position() {
    // From the official suite's error-indication block, pinned here per version.
    // "abc\xE3def": the \xE3 lead byte at position 4 lacks continuations.
    for v in UTF8_VERSIONS {
        let (a, b): (Value, i64) =
            eval(v, "local a, b = utf8.len('abc\\xE3def'); return a, b");
        assert!(
            matches!(a, Value::Nil),
            "len of a malformed string must fail (nil) under {v:?}, got {a:?}"
        );
        assert_eq!(b, 4, "len failure position must be the bad lead byte (4) under {v:?}");
    }

    // A stray continuation byte at the front reports position 1.
    for v in UTF8_VERSIONS {
        let (a, b): (Value, i64) =
            eval(v, "local a, b = utf8.len('\\x80hello'); return a, b");
        assert!(matches!(a, Value::Nil), "leading cont byte must fail under {v:?}");
        assert_eq!(b, 1, "leading-cont failure position must be 1 under {v:?}");
    }
}
