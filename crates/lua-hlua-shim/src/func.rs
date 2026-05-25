//! Closure hosting and the typed read/push traits.
//!
//! lua-rs native functions are bare `fn` pointers (an index into a per-state
//! registry), exactly like C's `lua_CFunction`. To host arbitrary Rust closures
//! that capture state we reproduce the trick `hlua` itself uses on the C API: a
//! single shared trampoline `fn` is registered as the C closure, the boxed
//! closure adapter lives in a thread-local registry, and the adapter's registry
//! index travels as the C closure's first upvalue. Execution only ever takes a
//! shared borrow of the registry, so nested native calls cannot deadlock it.

use std::cell::RefCell;
use std::collections::HashMap;
use std::marker::PhantomData;

use lua_types::{LuaError as VmError, LuaType};
use lua_vm::api;
use lua_vm::state::LuaState;

use crate::any::{
    push_any, push_hashable, read_any, read_map, read_sequence, string_bytes_at, AnyHashableLuaValue,
    AnyLuaValue,
};
use crate::{Lua, LuaError};

/// A type-erased native call: read args, invoke the user closure, push results.
pub(crate) type Adapter = Box<dyn Fn(&mut LuaState) -> Result<usize, VmError>>;

thread_local! {
    static REGISTRY: RefCell<Vec<Option<Adapter>>> = const { RefCell::new(Vec::new()) };
}

/// `LUA_REGISTRYINDEX`, mirrored from `lua-vm`; upvalue 1 sits one below it.
const LUA_REGISTRYINDEX: i32 = -(1_000_000) - 1000;

fn upvalue_index(n: i32) -> i32 {
    LUA_REGISTRYINDEX - n
}

pub(crate) fn registry_insert(adapter: Adapter) -> usize {
    REGISTRY.with(|cell| {
        let mut slots = cell.borrow_mut();
        match slots.iter().position(|slot| slot.is_none()) {
            Some(i) => {
                slots[i] = Some(adapter);
                i
            }
            None => {
                slots.push(Some(adapter));
                slots.len() - 1
            }
        }
    })
}

pub(crate) fn registry_remove(index: usize) {
    REGISTRY.with(|cell| {
        let mut slots = cell.borrow_mut();
        if index < slots.len() {
            slots[index] = None;
        }
    });
}

/// The one C function every hosted closure is registered as. It recovers its
/// adapter index from upvalue 1 and dispatches.
pub(crate) fn trampoline(state: &mut LuaState) -> Result<usize, VmError> {
    let index = api::to_integer_x(state, upvalue_index(1))
        .ok_or_else(|| VmError::runtime(format_args!("hlua-shim: closure upvalue missing")))?
        as usize;
    REGISTRY.with(|cell| {
        let slots = cell.borrow();
        match slots.get(index).and_then(|slot| slot.as_ref()) {
            Some(adapter) => adapter(state),
            None => Err(VmError::runtime(format_args!(
                "hlua-shim: closure {index} not registered"
            ))),
        }
    })
}

// ── reading native-function arguments off the stack ───────────────────────────

/// Convert a single positional argument at stack index `idx` into a Rust value.
pub trait LuaReadArg: Sized {
    fn read_arg(state: &mut LuaState, idx: i32) -> Result<Self, VmError>;
}

impl LuaReadArg for AnyLuaValue {
    fn read_arg(state: &mut LuaState, idx: i32) -> Result<Self, VmError> {
        Ok(read_any(state, idx))
    }
}

impl LuaReadArg for String {
    fn read_arg(state: &mut LuaState, idx: i32) -> Result<Self, VmError> {
        let bytes = string_bytes_at(state, idx)
            .ok_or_else(|| VmError::runtime(format_args!("expected string argument")))?;
        String::from_utf8(bytes)
            .map_err(|_| VmError::runtime(format_args!("string argument is not valid utf-8")))
    }
}

impl LuaReadArg for Vec<AnyLuaValue> {
    fn read_arg(state: &mut LuaState, idx: i32) -> Result<Self, VmError> {
        Ok(read_sequence(state, idx))
    }
}

impl LuaReadArg for HashMap<AnyHashableLuaValue, AnyLuaValue> {
    fn read_arg(state: &mut LuaState, idx: i32) -> Result<Self, VmError> {
        Ok(read_map(state, idx))
    }
}

macro_rules! read_arg_int {
    ($($ty:ty),*) => {$(
        impl LuaReadArg for $ty {
            fn read_arg(state: &mut LuaState, idx: i32) -> Result<Self, VmError> {
                let n = api::to_integer_x(state, idx)
                    .ok_or_else(|| VmError::runtime(format_args!("expected integer argument")))?;
                Ok(n as $ty)
            }
        }
    )*};
}
read_arg_int!(i32, u32, u16, i64, u64);

// ── pushing native-function results onto the stack ────────────────────────────

/// Push a Rust return value onto the stack, yielding the number of results.
pub trait PushToLua {
    fn push_to(self, state: &mut LuaState) -> Result<usize, VmError>;
}

impl PushToLua for () {
    fn push_to(self, _state: &mut LuaState) -> Result<usize, VmError> {
        Ok(0)
    }
}

impl PushToLua for bool {
    fn push_to(self, state: &mut LuaState) -> Result<usize, VmError> {
        api::push_boolean(state, self);
        Ok(1)
    }
}

impl PushToLua for String {
    fn push_to(self, state: &mut LuaState) -> Result<usize, VmError> {
        api::push_lstring(state, self.as_bytes())?;
        Ok(1)
    }
}

impl PushToLua for AnyLuaValue {
    fn push_to(self, state: &mut LuaState) -> Result<usize, VmError> {
        push_any(state, &self)?;
        Ok(1)
    }
}

impl PushToLua for Vec<AnyLuaValue> {
    fn push_to(self, state: &mut LuaState) -> Result<usize, VmError> {
        state.create_table(self.len() as i32, 0)?;
        let table = api::get_top(state);
        for (i, value) in self.iter().enumerate() {
            push_any(state, value)?;
            state.raw_seti(table, (i + 1) as i64)?;
        }
        Ok(1)
    }
}

impl PushToLua for HashMap<AnyHashableLuaValue, AnyLuaValue> {
    fn push_to(self, state: &mut LuaState) -> Result<usize, VmError> {
        state.create_table(0, self.len() as i32)?;
        let table = api::get_top(state);
        for (key, value) in &self {
            push_hashable(state, key)?;
            push_any(state, value)?;
            api::raw_set(state, table)?;
        }
        Ok(1)
    }
}

macro_rules! push_int {
    ($($ty:ty),*) => {$(
        impl PushToLua for $ty {
            fn push_to(self, state: &mut LuaState) -> Result<usize, VmError> {
                api::push_integer(state, self as i64);
                Ok(1)
            }
        }
    )*};
}
push_int!(i32, u32, u16, i64);

/// A closure returning `Result` never raises a Lua error: consumers record
/// failures out of band (e.g. authoscope's `state.error`) and the script keeps
/// running with `nil` substituted for the result. This matches hlua-badtouch's
/// observed behaviour and is what makes recorded-error handling work.
impl<T: PushToLua, E> PushToLua for Result<T, E> {
    fn push_to(self, state: &mut LuaState) -> Result<usize, VmError> {
        match self {
            Ok(value) => value.push_to(state),
            Err(_) => {
                api::push_nil(state);
                Ok(1)
            }
        }
    }
}

// ── the functionN family ──────────────────────────────────────────────────────

/// Installs a hosted closure as a global of the given name on `lua`.
pub trait SetValue {
    fn set_into(self, lua: &mut Lua<'_>, name: &str);
}

macro_rules! define_function {
    ($name:ident, $wrapper:ident, ($($arg:ident : $argty:ident),*), ($($idx:expr),*)) => {
        /// Wrapper produced by the matching `functionN`, mirroring hlua's API.
        pub struct $wrapper<F, $($argty,)* R> {
            f: F,
            _marker: PhantomData<fn($($argty,)*) -> R>,
        }

        /// Wrap a Rust closure so it can be stored as a Lua global via `Lua::set`.
        pub fn $name<F, $($argty,)* R>(f: F) -> $wrapper<F, $($argty,)* R>
        where
            F: Fn($($argty,)*) -> R + 'static,
            $($argty: LuaReadArg + 'static,)*
            R: PushToLua + 'static,
        {
            $wrapper { f, _marker: PhantomData }
        }

        impl<F, $($argty,)* R> SetValue for $wrapper<F, $($argty,)* R>
        where
            F: Fn($($argty,)*) -> R + 'static,
            $($argty: LuaReadArg + 'static,)*
            R: PushToLua + 'static,
        {
            fn set_into(self, lua: &mut Lua<'_>, name: &str) {
                let f = self.f;
                let adapter: Adapter = Box::new(move |state: &mut LuaState| {
                    $(let $arg = $argty::read_arg(state, $idx)?;)*
                    let result = f($($arg,)*);
                    result.push_to(state)
                });
                lua.install_closure(name, adapter);
            }
        }
    };
}

define_function!(function0, Function0, (), ());
define_function!(function1, Function1, (a0: A0), (1));
define_function!(function2, Function2, (a0: A0, a1: A1), (1, 2));
define_function!(function3, Function3, (a0: A0, a1: A1, a2: A2), (1, 2, 3));
define_function!(function4, Function4, (a0: A0, a1: A1, a2: A2, a3: A3), (1, 2, 3, 4));
define_function!(
    function5,
    Function5,
    (a0: A0, a1: A1, a2: A2, a3: A3, a4: A4),
    (1, 2, 3, 4, 5)
);
define_function!(
    function6,
    Function6,
    (a0: A0, a1: A1, a2: A2, a3: A3, a4: A4, a5: A5),
    (1, 2, 3, 4, 5, 6)
);

// ── reading Lua functions back out, and calling them ──────────────────────────

/// A handle to a global Lua function, kept by name so it can be re-fetched and
/// called. Mirrors `hlua::LuaFunction<L>` (single type parameter).
pub struct LuaFunction<L> {
    pub(crate) inner: L,
}

/// The concrete `L` we instantiate `LuaFunction` with: a live borrow of the VM
/// plus the global name to call.
pub struct FnHandle<'a> {
    pub(crate) state: &'a mut LuaState,
    pub(crate) name: String,
}

/// Arguments pushed for a Lua call, returning how many were pushed.
pub trait PushArgs {
    fn push_args(self, state: &mut LuaState) -> Result<usize, VmError>;
}

impl PushArgs for (AnyLuaValue, AnyLuaValue) {
    fn push_args(self, state: &mut LuaState) -> Result<usize, VmError> {
        push_any(state, &self.0)?;
        push_any(state, &self.1)?;
        Ok(2)
    }
}

impl PushArgs for (AnyLuaValue,) {
    fn push_args(self, state: &mut LuaState) -> Result<usize, VmError> {
        push_any(state, &self.0)?;
        Ok(1)
    }
}

/// Read a single returned value off the top of the stack.
pub trait FromTop: Sized {
    fn from_top(state: &mut LuaState) -> Self;
}

impl FromTop for AnyLuaValue {
    fn from_top(state: &mut LuaState) -> Self {
        read_any(state, -1)
    }
}

impl<'a> LuaFunction<FnHandle<'a>> {
    /// Call the function with the given argument tuple and read one result.
    pub fn call_with_args<V, A>(&mut self, args: A) -> Result<V, LuaError>
    where
        A: PushArgs,
        V: FromTop,
    {
        let state = &mut *self.inner.state;
        api::get_global(state, self.inner.name.as_bytes()).map_err(LuaError::from_vm)?;
        let nargs = args.push_args(state).map_err(LuaError::from_vm)?;
        state
            .protected_call(nargs as i32, 1, 0)
            .map_err(LuaError::from_vm)?;
        let value = V::from_top(state);
        api::set_top(state, -2).ok();
        Ok(value)
    }
}

// ── borrowing a Lua string without copying out of the VM ──────────────────────

/// Mirror of `hlua::StringInLua<L>`: owns the read string and derefs to `str`.
pub struct StringInLua<L> {
    pub(crate) value: String,
    pub(crate) _marker: PhantomData<L>,
}

impl<L> std::ops::Deref for StringInLua<L> {
    type Target = str;
    fn deref(&self) -> &str {
        &self.value
    }
}

// ── reading named globals (backs `Lua::get`) ──────────────────────────────────

/// Read a global named `name` from `lua`, mirroring `hlua`'s `get`.
pub trait FromLuaGlobal<'l>: Sized {
    fn from_lua_global<'lua>(lua: &'l mut Lua<'lua>, name: &str) -> Option<Self>;
}

impl<'l> FromLuaGlobal<'l> for AnyLuaValue {
    fn from_lua_global<'lua>(lua: &'l mut Lua<'lua>, name: &str) -> Option<Self> {
        let state = lua.state_mut();
        api::get_global(state, name.as_bytes()).ok()?;
        let value = read_any(state, -1);
        api::set_top(state, -2).ok();
        Some(value)
    }
}

impl<'l> FromLuaGlobal<'l> for String {
    fn from_lua_global<'lua>(lua: &'l mut Lua<'lua>, name: &str) -> Option<Self> {
        let state = lua.state_mut();
        api::get_global(state, name.as_bytes()).ok()?;
        let bytes = string_bytes_at(state, -1);
        api::set_top(state, -2).ok();
        String::from_utf8(bytes?).ok()
    }
}

impl<'l> FromLuaGlobal<'l> for StringInLua<()> {
    fn from_lua_global<'lua>(lua: &'l mut Lua<'lua>, name: &str) -> Option<Self> {
        let value: String = FromLuaGlobal::from_lua_global(lua, name)?;
        Some(StringInLua {
            value,
            _marker: PhantomData,
        })
    }
}

impl<'l> FromLuaGlobal<'l> for LuaFunction<FnHandle<'l>> {
    fn from_lua_global<'lua>(lua: &'l mut Lua<'lua>, name: &str) -> Option<Self> {
        let owned_name = name.to_string();
        let state = lua.state_mut();
        let ty = api::get_global(state, name.as_bytes()).ok()?;
        api::set_top(state, -2).ok();
        if ty != LuaType::Function {
            return None;
        }
        Some(LuaFunction {
            inner: FnHandle {
                state,
                name: owned_name,
            },
        })
    }
}
