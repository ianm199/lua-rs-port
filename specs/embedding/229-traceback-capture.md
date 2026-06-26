# Spec #229 — Stack-traceback capture in the embedding `Error`

Status: design, pre-implementation. The readable-message half shipped in 0.3.5
(`Error::Display` → `message_lossy`). This spec is the **traceback capture** half.
Reviewer focus: **don't pollute existing error messages / don't change behavior
for code that doesn't opt in** (the multiversion oracle and error-wording tests
assert exact message strings).

## Problem

When a host catches an `Error` from `Chunk::exec/eval` or `Function::call`, it
gets only the immediate error value — no `debug.traceback()` stack. The CLI has
tracebacks; the embedding API does not.

## Substrate (verified)

- `lua_vm::api::pcall_k(state, nargs, nresults, errfunc, ctx, k)` (`api.rs:2016`)
  **supports a message handler**: `errfunc` is a stack index of a handler function
  (0 = none) invoked **before the stack unwinds** (`do_.rs:133` `run_message_handler`).
- `lua_stdlib::auxlib::traceback(state, other, msg, level)` (`auxlib.rs:298`) is the
  `luaL_traceback` equivalent: walks the call stack and leaves the traceback string
  on `state`'s stack.
- The CLI pattern (`crates/lua-cli/src/interp.rs:91` `msghandler`, `:385` `docall`):
  push a C closure handler, call `pcall_k(..., errfunc = base, ...)`. The handler
  rewrites the error value into `"msg\nstack traceback:\n…"`.
- Today `Chunk::eval` (lib.rs:1659) and `Function::call` (lib.rs:2003) call
  `pcall_k(..., errfunc = 0, ...)` — no handler.
- `Error` (lib.rs:156): `{ inner: LuaError, _root: Option<RootedValue> }`.

## The hazard

The CLI handler **overwrites the error value** with `msg + traceback`. If we did
that on every `Function::call`/`eval`, the error *message* every host (and every
test) sees would gain a `\nstack traceback:\n…` tail — silently breaking
error-wording assertions. The multiversion oracle pcalls **inside Lua**, so its
inner error is unaffected; but `crates/lua-cli/tests/traceback_oracle.rs` and any
runtime test reading `Error` message text could shift, and embedders relying on a
clean message would regress.

## Design — opt-in capture via a side channel (message preserved)

Two requirements: (1) capture must run **pre-unwind** (only a message handler can),
(2) it must **not alter the error value/message**. So the handler builds the
traceback and stashes it in a side channel, returning the error value **unchanged**.
Off by default.

### API

```rust
impl Lua {
    /// Enable/disable capturing a stack traceback into Errors raised by
    /// protected calls on this instance. Off by default (zero cost, message
    /// unchanged). Opt-in because it walks the stack on every error.
    pub fn set_capture_tracebacks(&self, on: bool);
    pub fn captures_tracebacks(&self) -> bool;
}

impl Error {
    /// The captured `debug.traceback()` stack, if capture was enabled when this
    /// error was raised. The error *message* (`Display`, `message_lossy`) is
    /// unchanged whether or not capture is on.
    pub fn traceback(&self) -> Option<&str>;
}
```

### Mechanism

- Add `Error.traceback: Option<String>` (third field).
- Side channel: a `RefCell<Option<Vec<u8>>>` on `GlobalState` (e.g.
  `pending_traceback`) — **VM-layer change**, plus a `capture_tracebacks: Cell<bool>`
  flag.
- Embedding message handler `traceback_msghandler(state)`:
  - if `!state.global().capture_tracebacks` → return arg 1 unchanged (fast path);
  - else build the traceback (`auxlib::traceback(state, None, Some(msg_bytes), 1)`),
    move the resulting string into `global.pending_traceback`, **pop it**, and
    return arg 1 (the original error value) unchanged.
- `Chunk::eval/exec`, `Function::call`: when `capture_tracebacks` is on, push
  `traceback_msghandler` and pass its index as `errfunc`; otherwise keep
  `errfunc = 0` (no behavior change when off).
- `capture_error_in_state` (lib.rs:1007): after building the `Error`, take
  `global.pending_traceback` (clearing it) into `Error.traceback`.

This keeps `inner`/message pristine; the traceback is strictly additive.

## Alternatives considered

- **Always-on, overwrite the error value (CLI style)** — rejected: changes every
  error message, breaks wording assertions, no clean message/traceback split.
- **Post-`pcall` capture** (build traceback in the `Err` arm) — rejected: the stack
  has already unwound, so the traceback is empty/wrong. Must be a pre-unwind handler.

## Risks for the reviewer

1. The side-channel must be cleared on *every* path (success and failure), or a
   stale traceback could attach to a later error. capture_error_in_state taking +
   clearing is the single consumer; confirm no other error path bypasses it.
2. Re-entrancy: nested protected calls each install a handler; the innermost
   error's handler runs first and writes the side channel — confirm an outer
   handler doesn't overwrite the inner traceback before capture reads it. (Likely
   fine because capture reads immediately after each pcall returns, but verify.)
3. Coroutine errors: `auxlib::traceback` takes an `other` state for cross-thread —
   out of scope here (host coroutine = #230); document that captured tracebacks are
   for the main-thread call path in v1.
4. Adding a `GlobalState` field is a VM-layer change in a hot struct — confirm it
   doesn't perturb the dispatch/`legacy_for` hot path (it's cold-path only).

## Test plan

`crates/lua-rs-runtime/tests/traceback_capture.rs`:
- capture off (default): `Error::traceback()` is `None`; message text byte-identical
  to today (guards against message pollution).
- capture on: a Rust→Lua→Rust nested error yields a traceback naming the frames;
  message text still clean (no `stack traceback` substring in `message_lossy`).
- toggling on→off→on behaves; side channel doesn't leak a stale traceback to a
  later error.

Oracle gate: `multiversion_oracle` byte-identical (it pcalls inside Lua, must be
untouched), CLI `traceback_oracle` (16) green, full `cargo test -p omnilua` green.

## Open questions for the reviewer

- Is a `GlobalState` side channel the right home, or should the flag/slot live on
  `LuaInner` (runtime crate) to avoid a VM-layer change? (Handler runs in VM with
  only `&mut LuaState`, so it needs VM-reachable storage — hence GlobalState.)
- Should capture default on for `Chunk` (scripts) but off for `Function::call`
  (hot)? Proposed: uniformly off, opt-in, simplest + zero surprise.
