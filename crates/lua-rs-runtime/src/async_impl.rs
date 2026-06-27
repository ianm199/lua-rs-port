//! Async integration (the `async` feature): register Rust `async` host
//! functions callable from Lua, and drive Lua from Rust while awaiting them.
//!
//! Design: a thin layer over the **existing, tested coroutine machinery** — no
//! VM, GC, or `unsafe` changes. An async host function is a small Lua closure
//! that `coroutine.yield`s a per-function **capability token** plus its
//! arguments. The driver runs the target as a coroutine ([`Lua::create_thread`]
//! + [`Thread::resume`]/[`Thread::status`]); each time it sees a registered
//! token it looks up the matching Rust future-producer, `await`s it (with no VM
//! borrow held), and resumes the coroutine with the result. Because suspension
//! is an ordinary coroutine yield, GC-rooting of the suspended coroutine across
//! the `.await` is inherited from the coroutine path.
//!
//! The token is the address of a per-function owned allocation, surfaced to Lua
//! as a light-userdata. Lua code cannot fabricate a light-userdata, so a script
//! can only resolve callbacks whose tokens it legitimately received (the async
//! functions it was actually handed) — a guessed or forged value matches no
//! entry and is rejected. This matters for the untrusted-script use case.
//!
//! The public surface is `fn … -> impl Future` (not `async fn`) so the futures
//! are self-contained (they clone the needed handles rather than borrowing
//! `self`).
//!
//! Constraints: needs the `coroutine` stdlib (the `async` feature enables it); an
//! async function must be called inside a driver (`call_async`/`eval_async`/
//! `exec_async`), never synchronously; the executor is the caller's (the futures
//! are `!Send`, so a single-threaded executor such as tokio's `LocalSet`).

use std::future::Future;
use std::pin::Pin;
use std::rc::Rc;

use crate::{
    Chunk, Error, Function, FromLuaMulti, IntoLuaMulti, Lua, LuaError, LuaString, Result, Value,
    Variadic,
};

/// The producer half of an async host function: given the yielded argument
/// values it returns a future of the result values. `Rc` so the driver can
/// clone it out of the registry and drop the registry borrow before running it.
pub(crate) type AsyncCallback =
    Rc<dyn Fn(Lua, Vec<Value>) -> Pin<Box<dyn Future<Output = Result<Vec<Value>>>>>>;

/// One registered async host function. `id`'s heap address is the capability
/// token surfaced to Lua as a light-userdata; the `Box` keeps that address
/// reserved for the instance's lifetime so it can never alias a later one.
pub(crate) struct AsyncEntry {
    id: Box<u8>,
    callback: AsyncCallback,
}

/// Factory chunk: called with `(token, yield)`, returns a closure that yields
/// `(token, ...)`. It captures the genuine `coroutine.yield` as an upvalue so a
/// later reassignment of the global cannot bypass the async driver.
const ASYNC_FACTORY_SRC: &[u8] =
    b"local token, yield = ...\nreturn function(...) return yield(token, ...) end\n";

/// Wraps the target in a Lua closure so `coroutine.create` always receives a
/// Lua function — Lua 5.1's `coroutine.create` rejects a C/Rust function, so a
/// `call_async` on a [`Lua::create_function`] result would otherwise break only
/// on 5.1. The tail call preserves yields through the wrapper on every version.
const ASYNC_TRAMPOLINE_SRC: &[u8] = b"local target = ...\nreturn function(...) return target(...) end\n";

/// The token light-userdata for an entry's id allocation.
fn token_value(entry: &AsyncEntry) -> Value {
    Value::LightUserData(token_addr(entry) as *mut core::ffi::c_void)
}

fn token_addr(entry: &AsyncEntry) -> usize {
    &*entry.id as *const u8 as usize
}

/// Registry key for a captured genuine coroutine primitive. The leading `\0`
/// keeps it out of the way of ordinary string keys.
fn async_primitive_key(name: &str) -> Vec<u8> {
    let mut key = b"\0omnilua.async.".to_vec();
    key.extend_from_slice(name.as_bytes());
    key
}

impl Lua {
    /// Register a Rust `async` function callable from Lua. Calling the returned
    /// function from inside a driver ([`Function::call_async`] /
    /// [`Chunk::eval_async`]) suspends the running coroutine until the future
    /// resolves, then continues with its result.
    ///
    /// The function must be invoked within a driver; calling it on a plain
    /// synchronous path raises "attempt to yield from outside a coroutine".
    pub fn create_async_function<A, R, F, Fut>(&self, f: F) -> Result<Function>
    where
        A: FromLuaMulti + 'static,
        R: IntoLuaMulti + 'static,
        F: Fn(Lua, A) -> Fut + 'static,
        Fut: Future<Output = Result<R>> + 'static,
    {
        let callback: AsyncCallback =
            Rc::new(move |lua, raw_args| match A::from_lua_multi(raw_args, &lua) {
                Err(e) => Box::pin(async move { Err(e) }),
                Ok(args) => {
                    let fut = f(lua.clone(), args);
                    Box::pin(async move {
                        let result = fut.await?;
                        result.into_lua_multi(&lua)
                    })
                }
            });

        let entry = AsyncEntry { id: Box::new(0u8), callback };
        let token = token_value(&entry);
        self.inner.async_registry.borrow_mut().push(entry);

        let genuine_yield = self.async_coroutine_primitive("yield")?;
        let factory = self.load(ASYNC_FACTORY_SRC).into_function()?;
        factory.call((token, genuine_yield))
    }

    /// Capture the genuine `coroutine.create/resume/status/yield` into the
    /// script-inaccessible registry at construction, before any user code can
    /// reassign those globals. The async driver and wrappers use these captured
    /// functions, so global tampering — even by a prior chunk on a shared
    /// instance, or a sandbox that strips `coroutine` from the globals — cannot
    /// bypass or break async dispatch. Best-effort: the `async` feature enables
    /// `coroutine`, so the lookups succeed; if one ever didn't, the driver
    /// reports a clear error rather than misbehaving.
    pub(crate) fn capture_async_coroutine_primitives(&self) {
        for name in ["create", "resume", "status", "yield"] {
            if let Ok(func) = self.coroutine_builtin(name) {
                let _stored = self.set_named_registry_value(async_primitive_key(name), func);
            }
        }
    }

    /// Fetch a captured genuine coroutine primitive (see
    /// [`Lua::capture_async_coroutine_primitives`]).
    fn async_coroutine_primitive(&self, name: &str) -> Result<Function> {
        self.named_registry_value(async_primitive_key(name))
    }

    /// Clone out the callback whose token matches `addr`, if any. The registry
    /// borrow is released before the returned callback is run, so a callback may
    /// itself register further async functions.
    fn async_callback_for(&self, addr: usize) -> Option<AsyncCallback> {
        let registry = self.inner.async_registry.borrow();
        registry
            .iter()
            .find(|entry| token_addr(entry) == addr)
            .map(|entry| entry.callback.clone())
    }
}

impl Function {
    /// Call this function, awaiting any async host functions it invokes. The
    /// returned future drives the call to completion; run it on the caller's
    /// (single-threaded) executor.
    pub fn call_async<A, R>(&self, args: A) -> impl Future<Output = Result<R>>
    where
        A: IntoLuaMulti + 'static,
        R: FromLuaMulti + 'static,
    {
        let func = self.clone();
        async move {
            let lua = func.root.lua.clone();
            drive_async(lua, func, args).await
        }
    }
}

impl Chunk {
    /// Compile and run this chunk, awaiting any async host functions it invokes,
    /// returning its value(s). Compilation happens eagerly; the returned future
    /// drives execution.
    pub fn eval_async<R>(self) -> impl Future<Output = Result<R>>
    where
        R: FromLuaMulti + 'static,
    {
        let lua = self.lua.clone();
        let compiled = self.into_function();
        async move { drive_async(lua, compiled?, ()).await }
    }

    /// Like [`Chunk::eval_async`] but discards the result.
    pub fn exec_async(self) -> impl Future<Output = Result<()>> {
        self.eval_async::<()>()
    }
}

/// Run `func` as a coroutine, resolving each async suspension by awaiting the
/// matching registered future, until the coroutine finishes.
///
/// The `coroutine.create`/`resume`/`status` functions are captured once, at
/// entry, *before* the driven code runs — so a script that reassigns those
/// globals mid-run cannot make the driver misread a suspension as a return (it
/// drives through the captured genuine functions, not the live globals).
fn drive_async<A, R>(lua: Lua, func: Function, args: A) -> impl Future<Output = Result<R>>
where
    A: IntoLuaMulti + 'static,
    R: FromLuaMulti + 'static,
{
    async move {
        let create = lua.async_coroutine_primitive("create")?;
        let resume = lua.async_coroutine_primitive("resume")?;
        let status = lua.async_coroutine_primitive("status")?;
        let trampoline = lua.load(ASYNC_TRAMPOLINE_SRC).into_function()?;
        let wrapped: Function = trampoline.call(Value::Function(func))?;
        let co: Value = create.call(Value::Function(wrapped))?;
        let mut next: Vec<Value> = args.into_lua_multi(&lua)?;
        loop {
            let mut call_args = Vec::with_capacity(next.len() + 1);
            call_args.push(co.clone());
            call_args.append(&mut next);
            let mut returns = resume
                .call::<_, Variadic<Value>>(Variadic::from(call_args))?
                .into_vec();
            let ok = match returns.first() {
                Some(Value::Boolean(b)) => *b,
                _ => {
                    return Err(Error::from(LuaError::runtime(format_args!(
                        "coroutine.resume did not return a status boolean"
                    ))))
                }
            };
            returns.remove(0);
            if !ok {
                let err_val = returns.into_iter().next().unwrap_or(Value::Nil);
                let captured: Result<Error> = lua.with_state(|state| {
                    let raw = err_val.to_raw_for_lua(&lua, state)?;
                    Ok(lua.capture_error_in_state(state, LuaError::from_value(raw)))
                });
                return Err(match captured {
                    Ok(error) => error,
                    Err(error) => error,
                });
            }

            let state_name: LuaString = status.call(co.clone())?;
            match state_name.as_bytes()?.as_slice() {
                b"dead" => return R::from_lua_multi(returns, &lua),
                b"suspended" => {
                    let addr = match returns.first() {
                        Some(Value::LightUserData(p)) => *p as usize,
                        _ => {
                            return Err(Error::from(LuaError::runtime(format_args!(
                                "call_async target performed a plain coroutine.yield; only async \
                                 functions may suspend across an async driver"
                            ))))
                        }
                    };
                    let callback = lua.async_callback_for(addr).ok_or_else(|| {
                        Error::from(LuaError::runtime(format_args!(
                            "coroutine yielded a value that is not a known async token"
                        )))
                    })?;
                    returns.remove(0);
                    next = callback(lua.clone(), returns).await?;
                }
                other => {
                    return Err(Error::from(LuaError::runtime(format_args!(
                        "async coroutine reported an unexpected status: {}",
                        String::from_utf8_lossy(other)
                    ))))
                }
            }
        }
    }
}

// ──────────────────────────────────────────────────────────────────────────
// PORT STATUS
//   source:        (no C analog — Rust-native async embedding)
//   target_crate:  omnilua
//   confidence:    high
//   todos:         0
//   port_notes:    0
//   unsafe_blocks: 0
//   notes:         create_async_function + call_async/eval_async/exec_async,
//                  built on coroutine create_thread/resume/status + a yielded
//                  per-function capability token (light-userdata = address of an
//                  owned Box, unforgeable by Lua, version-invariant). Callbacks
//                  are Rc so the registry borrow drops before user code runs
//                  (re-entrant create_async_function safe). No VM/GC/unsafe;
//                  rooting inherited from the coroutine path. Feature `async`
//                  enables `coroutine`. Future errors propagate to the Rust
//                  caller (coroutine left suspended; not injected into pcall).
// ──────────────────────────────────────────────────────────────────────────
