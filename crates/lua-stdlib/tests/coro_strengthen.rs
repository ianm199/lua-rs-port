//! Behavioral net for the version-seam surface of the `coroutine.*` library.
//!
//! `coro_lib.rs` is the cold arg-checking / status-string / registration shell
//! around the (load-bearing) `lua-vm` resume/yield control-transfer machinery.
//! Most of its seams differ by Lua version, and several were thinly covered or
//! uncovered by the behavioral oracle before this file: the `running()` return
//! arity (5.1 `nil` vs 5.2+ `(thread, ismain)`), the `isyieldable` gate (5.3+),
//! the `close` gate + semantics (5.4+), the resume/yield error wording (which
//! changed between 5.1 and 5.2), and the suspended→dead status transition that
//! `close` drives.
//!
//! Every expectation here is pinned to the **reference binaries**
//! (`/tmp/lua-refs/bin/lua5.{1.5,2.4,3.6,4.7,5.0}`), never to the impl's own
//! output — these are an independent oracle, not a tautology. Two of them
//! (`v51_double_resume_running_message`, the 5.1 wording) FAILED against the
//! un-idiomatized baseline and caught a real cross-version bug, fixed in
//! `coro_lib.rs` (see the graduation note in that file's header). Thread
//! identities (addresses) are non-deterministic and are never asserted on —
//! only the observable arity, type, boolean, status string, and error wording.
//!
//! `omnilua` is a dev-dependency here (it depends on `lua-stdlib`, so it can
//! only be a dev-dep — see `Cargo.toml`), the same shape as `math_float_only.rs`.

use omnilua::{Lua, LuaVersion};

/// All five versions, in roster order.
const ALL: [LuaVersion; 5] = [
    LuaVersion::V51,
    LuaVersion::V52,
    LuaVersion::V53,
    LuaVersion::V54,
    LuaVersion::V55,
];

/// Evaluate `code` under `version` and return the `String` it produces. The
/// snippet is responsible for reducing the observed behavior to a single
/// string so that no non-deterministic value (a thread address) is ever
/// compared.
fn eval_str(version: LuaVersion, code: &str) -> String {
    let lua = Lua::new_versioned(version);
    lua.load(code)
        .eval::<String>()
        .unwrap_or_else(|e| panic!("eval of `{code}` failed under {version:?}: {e:?}"))
}

/// Assert the snippet's returned string equals `expected` under `version`.
fn assert_eval(version: LuaVersion, code: &str, expected: &str) {
    let got = eval_str(version, code);
    assert_eq!(
        got, expected,
        "under {version:?}, `{code}` returned {got:?}, expected {expected:?}"
    );
}

// ── coroutine.running() — return arity differs 5.1 vs 5.2+ ─────────────────────

/// `coroutine.running()` in the main thread returns `nil` (one value) on 5.1,
/// but `(thread, true)` on 5.2+. The `ismain` boolean is a 5.2 addition.
/// Reference-pinned: lua5.1.5 prints `nil`; lua5.2.4+ print `thread true`.
#[test]
fn running_in_main_arity_by_version() {
    // Snippet reports: <type-of-first> <tostring-of-second>.
    let code = "local a,b = coroutine.running(); return type(a) .. ' ' .. tostring(b)";
    assert_eval(LuaVersion::V51, code, "nil nil");
    for v in [
        LuaVersion::V52,
        LuaVersion::V53,
        LuaVersion::V54,
        LuaVersion::V55,
    ] {
        assert_eval(v, code, "thread true");
    }
}

/// Inside a coroutine, `coroutine.running()` returns the running thread plus a
/// boolean that is `false` (not the main thread) on 5.2+. On 5.1 it returns
/// only the thread (no boolean → `nil` for the second slot). Reference-pinned.
#[test]
fn running_in_coroutine_arity_by_version() {
    let code = "local out; \
                coroutine.wrap(function() \
                  local a,b = coroutine.running(); \
                  out = type(a) .. ' ' .. tostring(b) \
                end)(); \
                return out";
    assert_eval(LuaVersion::V51, code, "thread nil");
    for v in [
        LuaVersion::V52,
        LuaVersion::V53,
        LuaVersion::V54,
        LuaVersion::V55,
    ] {
        assert_eval(v, code, "thread false");
    }
}

// ── coroutine.isyieldable — present from 5.3, absent on 5.1/5.2 ────────────────

/// `coroutine.isyieldable` exists from 5.3 onward; it is `nil` on 5.1 and 5.2.
/// Reference-pinned: `type(coroutine.isyieldable)` is `"nil"` on 5.1.5/5.2.4
/// and `"function"` on 5.3.6/5.4.7/5.5.0.
#[test]
fn isyieldable_presence_by_version() {
    let code = "return type(coroutine.isyieldable)";
    assert_eval(LuaVersion::V51, code, "nil");
    assert_eval(LuaVersion::V52, code, "nil");
    for v in [LuaVersion::V53, LuaVersion::V54, LuaVersion::V55] {
        assert_eval(v, code, "function");
    }
}

/// Where present (5.3+), `isyieldable()` is `false` in the main thread and
/// `true` inside a coroutine. Reference-pinned against lua5.3.6/5.4.7/5.5.0.
#[test]
fn isyieldable_main_vs_coroutine() {
    let main_code = "return tostring(coroutine.isyieldable())";
    let coro_code = "local out; \
                     coroutine.wrap(function() \
                       out = tostring(coroutine.isyieldable()) \
                     end)(); \
                     return out";
    for v in [LuaVersion::V53, LuaVersion::V54, LuaVersion::V55] {
        assert_eval(v, main_code, "false");
        assert_eval(v, coro_code, "true");
    }
}

// ── coroutine.close — present from 5.4, absent on 5.1/5.2/5.3 ──────────────────

/// `coroutine.close` exists from 5.4 onward; it is `nil` on 5.1/5.2/5.3.
/// Reference-pinned: `type(coroutine.close)` is `"nil"` below 5.4 and
/// `"function"` on 5.4.7/5.5.0.
#[test]
fn close_presence_by_version() {
    let code = "return type(coroutine.close)";
    for v in [LuaVersion::V51, LuaVersion::V52, LuaVersion::V53] {
        assert_eval(v, code, "nil");
    }
    for v in [LuaVersion::V54, LuaVersion::V55] {
        assert_eval(v, code, "function");
    }
}

/// `coroutine.close` on a *dead* coroutine returns `true`. Reference-pinned
/// against lua5.4.7/5.5.0.
#[test]
fn close_dead_returns_true() {
    let code = "local co = coroutine.create(function() end); \
                coroutine.resume(co); \
                return tostring(coroutine.close(co))";
    for v in [LuaVersion::V54, LuaVersion::V55] {
        assert_eval(v, code, "true");
    }
}

/// `coroutine.close` on a *suspended* coroutine returns `true` and drives the
/// status from `suspended` to `dead`. This pins the status TRANSITION that
/// `close` causes, not just its return value. Reference-pinned against
/// lua5.4.7/5.5.0 (both print `suspended` / `true` / `dead`).
#[test]
fn close_suspended_transitions_to_dead() {
    let code = "local co = coroutine.create(function() coroutine.yield() end); \
                coroutine.resume(co); \
                local before = coroutine.status(co); \
                local ok = coroutine.close(co); \
                local after = coroutine.status(co); \
                return before .. ' ' .. tostring(ok) .. ' ' .. after";
    for v in [LuaVersion::V54, LuaVersion::V55] {
        assert_eval(v, code, "suspended true dead");
    }
}

/// `coroutine.close` of a *normal* coroutine (one that resumed the closer and
/// is waiting) raises `"cannot close a normal coroutine"`. Pinned via `pcall`
/// + `string.find` so the location prefix is not asserted. Reference-pinned
/// against lua5.4.7/5.5.0.
#[test]
fn close_normal_coroutine_errors() {
    let code = "local main = coroutine.running(); \
                local co = coroutine.create(function() \
                  local ok, msg = pcall(coroutine.close, main); \
                  return tostring(ok) .. '|' .. (string.find(msg, 'normal coroutine') ~= nil and 'normal' or msg) \
                end); \
                local _, r = coroutine.resume(co); \
                return r";
    for v in [LuaVersion::V54, LuaVersion::V55] {
        assert_eval(v, code, "false|normal");
    }
}

/// `coroutine.close` of the *running* coroutine (from inside itself):
/// on 5.4 it is an error (`"cannot close a running coroutine"`, returned via
/// `pcall` as `false, msg`); on 5.5 the self-close UNWINDS the running
/// coroutine (the `LuaThreadClose` path), so the body after the `close` call
/// never runs and the resume returns no extra values. This pins the 5.4↔5.5
/// divergence. Reference-pinned.
#[test]
fn close_running_self_5_4_errors_5_5_unwinds() {
    // 5.4: pcall(close, self) returns false + a "running coroutine" message,
    // so the body runs to completion and yields the marker string.
    let code_54 = "local co; co = coroutine.create(function() \
                     local ok, msg = pcall(coroutine.close, co); \
                     return tostring(ok) .. '|' .. (string.find(msg, 'running coroutine') ~= nil and 'running' or msg) \
                   end); \
                   local _, r = coroutine.resume(co); \
                   return tostring(r)";
    assert_eval(LuaVersion::V54, code_54, "false|running");

    // 5.5: close(self) unwinds the coroutine. The marker set BEFORE the close
    // survives; nothing after it runs. The outer resume reports success with no
    // post-close value, and the coroutine ends up dead.
    let code_55 = "local marker = 'pre'; \
                   local co; co = coroutine.create(function() \
                     marker = 'entered'; \
                     coroutine.close(co); \
                     marker = 'after-close' \
                   end); \
                   coroutine.resume(co); \
                   return marker .. ' ' .. coroutine.status(co)";
    assert_eval(LuaVersion::V55, code_55, "entered dead");
}

// ── resume errors — wording changed between 5.1 and 5.2 ────────────────────────

/// Resume of a *dead* coroutine returns `false, "cannot resume dead coroutine"`
/// on EVERY version. Pinned exactly (no location prefix on this message).
/// Reference-pinned against all five binaries.
#[test]
fn resume_dead_message_all_versions() {
    let code = "local co = coroutine.create(function() end); \
                coroutine.resume(co); \
                local ok, msg = coroutine.resume(co); \
                return tostring(ok) .. '|' .. msg";
    for v in ALL {
        assert_eval(v, code, "false|cannot resume dead coroutine");
    }
}

/// Double-resume of a *running* coroutine (it resumes itself): on 5.1 the
/// message is `"cannot resume running coroutine"`; from 5.2 it became
/// `"cannot resume non-suspended coroutine"`. This is the 5.1-vs-5.2+ wording
/// seam and it caught a real bug — the un-idiomatized impl emitted the 5.2+
/// wording on 5.1. The outer resume succeeds (`true`); the inner self-resume
/// fails (`false, <msg>`). Reference-pinned against lua5.1.5 vs lua5.2.4+.
#[test]
fn double_resume_running_message_by_version() {
    // The coroutine returns the inner (ok, msg); the outer resume wraps that.
    let code = "local co; co = coroutine.create(function() \
                  local ok, msg = coroutine.resume(co); \
                  return tostring(ok) .. '|' .. msg \
                end); \
                local _, inner = coroutine.resume(co); \
                return inner";
    assert_eval(LuaVersion::V51, code, "false|cannot resume running coroutine");
    for v in [
        LuaVersion::V52,
        LuaVersion::V53,
        LuaVersion::V54,
        LuaVersion::V55,
    ] {
        assert_eval(v, code, "false|cannot resume non-suspended coroutine");
    }
}

// ── wrap — error re-raise in the caller ────────────────────────────────────────

/// `coroutine.wrap` returns a function that re-RAISES a coroutine error in the
/// caller (unlike `resume`, which returns `false, err`). A `pcall` around the
/// wrapped call therefore catches the original error value. Reference-pinned:
/// every version surfaces `boom` to the caller's `pcall`.
#[test]
fn wrap_reraises_error_in_caller() {
    let code = "local f = coroutine.wrap(function() error('boom', 0) end); \
                local ok, msg = pcall(f); \
                return tostring(ok) .. '|' .. tostring(msg)";
    for v in ALL {
        assert_eval(v, code, "false|boom");
    }
}

/// A wrapped coroutine that yields then returns: the wrapper forwards yielded
/// values on the first call and returned values on the next, exactly like the
/// `resume` results minus the leading `true`. Reference-pinned.
#[test]
fn wrap_forwards_yield_then_return() {
    let code = "local f = coroutine.wrap(function() \
                  coroutine.yield('y1'); \
                  return 'r1' \
                end); \
                local a = f(); \
                local b = f(); \
                return a .. ' ' .. b";
    for v in ALL {
        assert_eval(v, code, "y1 r1");
    }
}

// ── status transitions across a yield ──────────────────────────────────────────

/// The full status lifecycle of a coroutine across a yield: `suspended` before
/// the first resume, `running` while it executes (observed from inside),
/// `suspended` again after it yields, then `dead` after it returns. While a
/// coroutine resumes a child, the PARENT reads as `normal`. This pins the
/// transition table the cold `aux_status` mapping produces. Reference-pinned
/// against all five binaries.
#[test]
fn status_transitions_across_yield() {
    let code = "local self_status; \
                local co = coroutine.create(function() \
                  self_status = coroutine.status(coroutine.running()); \
                  coroutine.yield() \
                end); \
                local s0 = coroutine.status(co); \
                coroutine.resume(co); \
                local s1 = coroutine.status(co); \
                coroutine.resume(co); \
                local s2 = coroutine.status(co); \
                return s0 .. ' ' .. self_status .. ' ' .. s1 .. ' ' .. s2";
    for v in ALL {
        assert_eval(v, code, "suspended running suspended dead");
    }
}

/// A parent coroutine that is currently resuming a child reads as `normal`
/// from the child's perspective. This pins the `COS_NORM` branch of
/// `aux_status` (the parent is active but neither running nor suspended).
/// Reference-pinned against all five binaries.
#[test]
fn parent_status_is_normal_while_child_runs() {
    let code = "local parent_status; \
                local parent = coroutine.create(function() \
                  local child = coroutine.create(function(p) \
                    parent_status = coroutine.status(p) \
                  end); \
                  coroutine.resume(child, coroutine.running()) \
                end); \
                coroutine.resume(parent); \
                return parent_status";
    for v in ALL {
        assert_eval(v, code, "normal");
    }
}
