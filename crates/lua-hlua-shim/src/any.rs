//! The dynamically-typed Lua value enums exposed by `hlua`, reproduced with
//! identical shape and derives so that consumer code which pattern-matches on
//! the variants compiles unchanged.
//!
//! The free functions in this module marshal these values across the lua-rs
//! stack using only the public `lua-vm` C-style API, so no `unsafe` is needed.

use std::collections::HashMap;

use lua_types::{LuaError as VmError, LuaType};
use lua_vm::api;
use lua_vm::state::LuaState;

/// A byte string that may not be valid UTF-8, mirroring `hlua::AnyLuaString`.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct AnyLuaString(pub Vec<u8>);

/// Any Lua value, mirroring `hlua::AnyLuaValue` field-for-field.
#[derive(Clone, Debug, PartialEq)]
pub enum AnyLuaValue {
    LuaString(String),
    LuaAnyString(AnyLuaString),
    LuaNumber(f64),
    LuaBoolean(bool),
    LuaArray(Vec<(AnyLuaValue, AnyLuaValue)>),
    LuaNil,
    LuaOther,
}

/// The hashable projection used for Lua table keys, mirroring
/// `hlua::AnyHashableLuaValue`. Numbers narrow to `i32` exactly as upstream.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum AnyHashableLuaValue {
    LuaString(String),
    LuaAnyString(AnyLuaString),
    LuaNumber(i32),
    LuaBoolean(bool),
    LuaArray(Vec<(AnyHashableLuaValue, AnyHashableLuaValue)>),
    LuaNil,
    LuaOther,
}

/// Push an `AnyLuaValue` onto the top of the stack.
pub(crate) fn push_any(state: &mut LuaState, value: &AnyLuaValue) -> Result<(), VmError> {
    match value {
        AnyLuaValue::LuaNil | AnyLuaValue::LuaOther => api::push_nil(state),
        AnyLuaValue::LuaBoolean(b) => api::push_boolean(state, *b),
        AnyLuaValue::LuaNumber(n) => api::push_number(state, *n),
        AnyLuaValue::LuaString(s) => {
            api::push_lstring(state, s.as_bytes())?;
        }
        AnyLuaValue::LuaAnyString(s) => {
            api::push_lstring(state, &s.0)?;
        }
        AnyLuaValue::LuaArray(pairs) => {
            state.create_table(0, pairs.len() as i32)?;
            let table = api::get_top(state);
            for (k, v) in pairs {
                push_any(state, k)?;
                push_any(state, v)?;
                api::raw_set(state, table)?;
            }
        }
    }
    Ok(())
}

/// Push an `AnyHashableLuaValue` onto the top of the stack.
pub(crate) fn push_hashable(
    state: &mut LuaState,
    value: &AnyHashableLuaValue,
) -> Result<(), VmError> {
    match value {
        AnyHashableLuaValue::LuaNil | AnyHashableLuaValue::LuaOther => api::push_nil(state),
        AnyHashableLuaValue::LuaBoolean(b) => api::push_boolean(state, *b),
        AnyHashableLuaValue::LuaNumber(n) => api::push_integer(state, *n as i64),
        AnyHashableLuaValue::LuaString(s) => {
            api::push_lstring(state, s.as_bytes())?;
        }
        AnyHashableLuaValue::LuaAnyString(s) => {
            api::push_lstring(state, &s.0)?;
        }
        AnyHashableLuaValue::LuaArray(pairs) => {
            state.create_table(0, pairs.len() as i32)?;
            let table = api::get_top(state);
            for (k, v) in pairs {
                push_hashable(state, k)?;
                push_hashable(state, v)?;
                api::raw_set(state, table)?;
            }
        }
    }
    Ok(())
}

/// Read the value at stack index `idx` as an `AnyLuaValue`.
pub(crate) fn read_any(state: &mut LuaState, idx: i32) -> AnyLuaValue {
    match api::lua_type_at(state, idx) {
        LuaType::None | LuaType::Nil => AnyLuaValue::LuaNil,
        LuaType::Boolean => AnyLuaValue::LuaBoolean(api::to_boolean(state, idx)),
        LuaType::Number => {
            let n = api::to_number_x(state, idx).expect("Number value must convert to f64");
            AnyLuaValue::LuaNumber(n)
        }
        LuaType::String => read_string_value(state, idx),
        LuaType::Table => AnyLuaValue::LuaArray(read_table_pairs(state, idx)),
        _ => AnyLuaValue::LuaOther,
    }
}

/// Read the value at stack index `idx` as an `AnyHashableLuaValue`.
pub(crate) fn read_hashable(state: &mut LuaState, idx: i32) -> AnyHashableLuaValue {
    match read_any(state, idx) {
        AnyLuaValue::LuaNil => AnyHashableLuaValue::LuaNil,
        AnyLuaValue::LuaBoolean(b) => AnyHashableLuaValue::LuaBoolean(b),
        AnyLuaValue::LuaNumber(n) => AnyHashableLuaValue::LuaNumber(n as i32),
        AnyLuaValue::LuaString(s) => AnyHashableLuaValue::LuaString(s),
        AnyLuaValue::LuaAnyString(s) => AnyHashableLuaValue::LuaAnyString(s),
        AnyLuaValue::LuaArray(_) | AnyLuaValue::LuaOther => AnyHashableLuaValue::LuaOther,
    }
}

/// Read a string-typed stack slot, preserving raw bytes when not valid UTF-8.
fn read_string_value(state: &mut LuaState, idx: i32) -> AnyLuaValue {
    let bytes = string_bytes_at(state, idx).expect("String value must have bytes");
    match String::from_utf8(bytes) {
        Ok(s) => AnyLuaValue::LuaString(s),
        Err(e) => AnyLuaValue::LuaAnyString(AnyLuaString(e.into_bytes())),
    }
}

/// Borrow the bytes of a string-typed stack slot without numeric coercion.
pub(crate) fn string_bytes_at(state: &mut LuaState, idx: i32) -> Option<Vec<u8>> {
    let s = api::to_lua_string(state, idx).ok()??;
    Some(s.as_bytes().to_vec())
}

/// Iterate a table at `idx` into key/value pairs, restoring the stack.
fn read_table_pairs(state: &mut LuaState, idx: i32) -> Vec<(AnyLuaValue, AnyLuaValue)> {
    let table = api::abs_index(state, idx);
    let mut pairs = Vec::new();
    api::push_nil(state);
    while api::next(state, table).unwrap_or(false) {
        let key = read_any(state, -2);
        let value = read_any(state, -1);
        pairs.push((key, value));
        api::set_top(state, -2).ok();
    }
    pairs
}

/// Read a table at `idx` as an ordered list of its 1..n integer-keyed values.
pub(crate) fn read_sequence(state: &mut LuaState, idx: i32) -> Vec<AnyLuaValue> {
    let table = api::abs_index(state, idx);
    let mut out = Vec::new();
    let mut i = 1i64;
    loop {
        let t = match api::get_i(state, table, i) {
            Ok(t) => t,
            Err(_) => break,
        };
        if t == LuaType::Nil {
            api::set_top(state, -2).ok();
            break;
        }
        out.push(read_any(state, -1));
        api::set_top(state, -2).ok();
        i += 1;
    }
    out
}

/// Read a table at `idx` into a `HashMap` keyed by hashable Lua values.
pub(crate) fn read_map(
    state: &mut LuaState,
    idx: i32,
) -> HashMap<AnyHashableLuaValue, AnyLuaValue> {
    let table = api::abs_index(state, idx);
    let mut map = HashMap::new();
    api::push_nil(state);
    while api::next(state, table).unwrap_or(false) {
        let key = read_hashable(state, -2);
        let value = read_any(state, -1);
        map.insert(key, value);
        api::set_top(state, -2).ok();
    }
    map
}
