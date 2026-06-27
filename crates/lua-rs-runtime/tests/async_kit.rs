//! Deterministic dogfood harness for the `async` feature.
//!
//! No tokio, no sockets, no timers: a std-only `block_on` (built from
//! `std::task::Wake`, zero `unsafe`) drives the driver future, and `YieldOnce`
//! is a future that returns `Pending` exactly once before `Ready` — forcing a
//! genuine coroutine suspend/resume cycle that reproduces 100% of the time.

#![cfg(feature = "async")]

use std::cell::Cell;
use std::future::Future;
use std::pin::Pin;
use std::rc::Rc;
use std::sync::Arc;
use std::task::{Context, Poll, Wake, Waker};

use omnilua::{Lua, LuaError, LuaVersion};

/// Wakes by unparking the thread that called `block_on`.
struct ThreadWaker(std::thread::Thread);

impl Wake for ThreadWaker {
    fn wake(self: Arc<Self>) {
        self.0.unpark();
    }
    fn wake_by_ref(self: &Arc<Self>) {
        self.0.unpark();
    }
}

/// A minimal single-threaded executor — poll, park until woken, repeat.
fn block_on<F: Future>(future: F) -> F::Output {
    let waker: Waker = Arc::new(ThreadWaker(std::thread::current())).into();
    let mut cx = Context::from_waker(&waker);
    let mut future = Box::pin(future);
    loop {
        match future.as_mut().poll(&mut cx) {
            Poll::Ready(value) => return value,
            Poll::Pending => std::thread::park(),
        }
    }
}

/// Returns `Pending` once (waking itself), then `Ready(value)` — a deterministic
/// stand-in for "a real future that isn't ready yet".
struct YieldOnce<T> {
    value: Option<T>,
    yielded: bool,
}

impl<T: Unpin> Future for YieldOnce<T> {
    type Output = T;
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<T> {
        let this = self.get_mut();
        if this.yielded {
            Poll::Ready(this.value.take().expect("YieldOnce polled after completion"))
        } else {
            this.yielded = true;
            cx.waker().wake_by_ref();
            Poll::Pending
        }
    }
}

fn yield_once<T>(value: T) -> YieldOnce<T> {
    YieldOnce { value: Some(value), yielded: false }
}

#[test]
fn async_fn_with_ready_value() {
    let lua = Lua::new();
    let f = lua
        .create_async_function(|_, n: i64| async move { Ok(n * 2) })
        .unwrap();
    lua.globals().set("double", f).unwrap();
    let out: i64 = block_on(lua.load("return double(21)").eval_async()).unwrap();
    assert_eq!(out, 42);
}

#[test]
fn async_fn_genuinely_suspends_and_resumes() {
    let lua = Lua::new();
    let ran = Rc::new(Cell::new(0));
    let ran2 = ran.clone();
    let f = lua
        .create_async_function(move |_, n: i64| {
            let ran = ran2.clone();
            async move {
                let v = yield_once(n + 1).await;
                ran.set(ran.get() + 1);
                Ok(v)
            }
        })
        .unwrap();
    lua.globals().set("inc_async", f).unwrap();
    let out: i64 = block_on(lua.load("return inc_async(41)").eval_async()).unwrap();
    assert_eq!(out, 42);
    assert_eq!(ran.get(), 1, "the pending future must have actually been awaited");
}

#[test]
fn sequential_awaits_thread_values() {
    let lua = Lua::new();
    let add = lua
        .create_async_function(|_, (a, b): (i64, i64)| async move { Ok(yield_once(a + b).await) })
        .unwrap();
    lua.globals().set("add_async", add).unwrap();
    let out: i64 = block_on(
        lua.load("local x = add_async(1, 2); local y = add_async(x, 10); return y")
            .eval_async(),
    )
    .unwrap();
    assert_eq!(out, 13);
}

#[test]
fn call_async_on_the_function_directly() {
    let lua = Lua::new();
    let mul = lua
        .create_async_function(|_, (a, b): (i64, i64)| async move { Ok(yield_once(a * b).await) })
        .unwrap();
    let out: i64 = block_on(mul.call_async((6, 7))).unwrap();
    assert_eq!(out, 42);
}

#[test]
fn exec_async_runs_side_effects() {
    let lua = Lua::new();
    let bump = lua
        .create_async_function(|lua, ()| async move {
            let n: i64 = lua.globals().get("counter").unwrap();
            lua.globals().set("counter", yield_once(n + 1).await).unwrap();
            Ok(())
        })
        .unwrap();
    lua.globals().set("bump", bump).unwrap();
    lua.globals().set("counter", 0i64).unwrap();
    block_on(lua.load("bump(); bump(); bump()").exec_async()).unwrap();
    let counter: i64 = lua.globals().get("counter").unwrap();
    assert_eq!(counter, 3);
}

#[test]
fn error_from_future_propagates_to_caller() {
    let lua = Lua::new();
    let boom = lua
        .create_async_function(|_, ()| async move {
            let _ = yield_once(()).await;
            Err::<i64, _>(LuaError::runtime(format_args!("boom")).into())
        })
        .unwrap();
    lua.globals().set("boom", boom).unwrap();
    let result: Result<i64, _> = block_on(lua.load("return boom()").eval_async());
    assert!(result.is_err(), "a future error must surface to the async caller");
}

#[test]
fn async_fn_called_synchronously_errors() {
    let lua = Lua::new();
    let f = lua
        .create_async_function(|_, ()| async move { Ok(1i64) })
        .unwrap();
    lua.globals().set("a", f).unwrap();
    let result = lua.load("return a()").exec();
    assert!(
        result.is_err(),
        "calling an async function on a synchronous path must error (yield outside coroutine)"
    );
}

#[test]
fn plain_yield_in_target_is_rejected() {
    let lua = Lua::new();
    let result: Result<i64, _> = block_on(lua.load("return coroutine.yield(99)").eval_async());
    assert!(
        result.is_err(),
        "a plain coroutine.yield (no async marker) must be rejected by the driver"
    );
}

#[test]
fn async_works_on_float_only_versions() {
    for version in [LuaVersion::V51, LuaVersion::V52] {
        let lua = Lua::new_versioned(version);
        let f = lua
            .create_async_function(|_, n: i64| async move { Ok(yield_once(n + 1).await) })
            .unwrap();
        lua.globals().set("inc", f).unwrap();
        let out: i64 = block_on(lua.load("return inc(41)").eval_async()).unwrap();
        assert_eq!(out, 42, "async must work on the float-only version {version:?}");
    }
}

#[test]
fn callback_may_register_another_async_function() {
    let lua = Lua::new();
    let registered = Rc::new(Cell::new(false));
    let flag = registered.clone();
    let outer = lua
        .create_async_function(move |lua, ()| {
            let made = lua
                .create_async_function(|_, ()| async move { Ok(()) })
                .is_ok();
            flag.set(made);
            async move { Ok(yield_once(7i64).await) }
        })
        .unwrap();
    lua.globals().set("outer", outer).unwrap();
    let out: i64 = block_on(lua.load("return outer()").eval_async()).unwrap();
    assert_eq!(out, 7);
    assert!(
        registered.get(),
        "a callback must be able to register another async function without a borrow panic"
    );
}

#[test]
fn reassigning_coroutine_yield_does_not_bypass_async() {
    let lua = Lua::new();
    let ran = Rc::new(Cell::new(false));
    let flag = ran.clone();
    let f = lua
        .create_async_function(move |_, ()| {
            let flag = flag.clone();
            async move {
                flag.set(true);
                Ok(yield_once(7i64).await)
            }
        })
        .unwrap();
    lua.globals().set("a", f).unwrap();
    let out: i64 = block_on(
        lua.load("coroutine.yield = function() return 999 end; return a()")
            .eval_async(),
    )
    .unwrap();
    assert_eq!(out, 7, "the wrapper captured the genuine yield; the fake must not win");
    assert!(ran.get(), "the real future must have run");
}

#[test]
fn reassigning_coroutine_status_does_not_break_driver() {
    let lua = Lua::new();
    let f = lua
        .create_async_function(|_, ()| async move { Ok(yield_once(5i64).await) })
        .unwrap();
    lua.globals().set("a", f).unwrap();
    let out: i64 = block_on(
        lua.load("coroutine.status = function() return 'dead' end; return a()")
            .eval_async(),
    )
    .unwrap();
    assert_eq!(out, 5, "the driver uses captured status, immune to the fake 'dead'");
}

#[test]
fn call_async_on_rust_function_works_on_5_1() {
    let lua = Lua::new_versioned(LuaVersion::V51);
    let f = lua.create_function(|_, n: i64| Ok(n + 1)).unwrap();
    let out: i64 = block_on(f.call_async(41i64)).unwrap();
    assert_eq!(out, 42, "call_async on a Rust function must work on 5.1 (coroutine.create wrap)");
}

#[test]
fn lua_errors_from_async_propagate_and_stay_rooted() {
    let lua = Lua::new();
    let err = block_on(lua.load("error('boom')").exec_async()).unwrap_err();
    lua.gc_collect();
    assert!(
        format!("{err}").contains("boom"),
        "a string error payload must survive GC and read back"
    );

    let table_err: Result<i64, _> = block_on(lua.load("error({ code = 7 })").eval_async());
    assert!(table_err.is_err(), "a table error must propagate as Err, not panic");
    lua.gc_collect();
}

#[test]
fn prior_global_tampering_on_shared_instance_does_not_break_async() {
    let lua = Lua::new();
    let f = lua
        .create_async_function(|_, ()| async move { Ok(yield_once(5i64).await) })
        .unwrap();
    lua.globals().set("a", f).unwrap();
    lua.load(
        "coroutine.status = function() return 'dead' end\n\
         coroutine.create = function() error('nope') end\n\
         coroutine.resume = function() return false end\n\
         coroutine.yield = function() return 0 end",
    )
    .exec()
    .unwrap();
    let out: i64 = block_on(lua.load("return a()").eval_async()).unwrap();
    assert_eq!(
        out, 5,
        "async uses coroutine primitives captured at construction, immune to prior tampering"
    );
}

// ──────────────────────────────────────────────────────────────────────────
// PORT STATUS
//   source:        (no C analog — async dogfood harness)
//   target_crate:  omnilua
//   confidence:    high
//   todos:         0
//   port_notes:    0
//   unsafe_blocks: 0
//   notes:         Deterministic in-memory executor (std::task::Wake, no unsafe)
//                  + YieldOnce (pending-once) future. Exercises ready/suspend/
//                  sequential/call_async/exec_async/error/sync-misuse/plain-yield
//                  paths with no tokio or sockets.
// ──────────────────────────────────────────────────────────────────────────
