//! Lua table — canonical implementation now lives in `lua-types::table`.
//!
//! This file is a thin re-export for compatibility with workspace
//! consumers (`lua-lex`, `lua-vm::trace_impls`) that previously
//! imported `lua_vm::table::LuaTable`. The interesting code has moved
//! to `crates/lua-types/src/table.rs`; see the doc comment there.

pub use lua_types::table::{LuaTable, TableFlags, TableNode, TableSlotRef};

// ──────────────────────────────────────────────────────────────────────────
// PORT STATUS
//   source:        src/ltable.c  (995 lines, 28 functions)
//   target_crate:  lua-types  (canonical impl)
//   confidence:    high
//   todos:         0
//   port_notes:    0
//   unsafe_blocks: 0
//   notes:         Canonical LuaTable lives in lua-types/src/table.rs and
//                  is reachable through LuaValue::Table(GcRef<LuaTable>).
//                  This file exists only as a re-export shim so legacy
//                  `lua_vm::table::LuaTable` imports keep resolving.
// ──────────────────────────────────────────────────────────────────────────
