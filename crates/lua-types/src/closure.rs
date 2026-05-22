//! `LuaClosure` — the function variant of `LuaValue`. Three sub-kinds:
//! Lua closure (compiled Proto + upvalues), C closure (function pointer +
//! upvalues), light C function (function pointer, no upvalues).

use crate::gc::GcRef;
use crate::proto::LuaProto;
use crate::upval::UpVal;
use crate::value::LuaValue;

/// Placeholder Phase-A C-function pointer type. Real signature
/// (`fn(&mut LuaState) -> Result<usize, LuaError>`) lives in `lua-vm`;
/// lua-types can't reference `LuaState` without a circular dep. Compiler-
/// fixer pass replaces every use of this placeholder with the real type.
pub type LuaCFnPtr = fn() -> i32;

#[derive(Debug, Clone)]
pub enum LuaClosure {
    Lua(GcRef<LuaLClosure>),
    C(GcRef<LuaCClosure>),
    LightC(LuaCFnPtr),
}

#[derive(Debug)]
pub struct LuaLClosure {
    pub proto: GcRef<LuaProto>,
    pub upvals: Vec<GcRef<UpVal>>,
}

#[derive(Debug)]
pub struct LuaCClosure {
    pub func: LuaCFnPtr,
    pub upvalues: Vec<LuaValue>,
}

impl LuaLClosure {
    pub fn placeholder() -> Self {
        LuaLClosure {
            proto: GcRef::new(LuaProto::placeholder()),
            upvals: Vec::new(),
        }
    }
}
