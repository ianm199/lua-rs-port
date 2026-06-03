//! Lua 5.4 standard library — runtime stdlib crate.
//!
//! Each module corresponds to one C source under reference/lua-5.4.7/src/.
//! See ANALYSES/file_deps.txt for the mapping.

pub mod auxlib;
pub mod base;
pub mod bit32_lib;
pub mod coro_lib;
pub mod debug_lib;
pub mod init;
pub mod io_lib;
pub mod loadlib;
pub mod math_lib;
pub mod os_lib;
pub mod sandbox;
pub mod state_stub;
pub mod string_lib;
pub mod table_lib;
pub mod utf8_lib;

// ──────────────────────────────────────────────────────────────────────────
// PORT STATUS
//   source:        (module aggregator)
//   target_crate:  lua-stdlib
//   confidence:    high
//   todos:         0
//   port_notes:    0
//   unsafe_blocks: 0
//   notes:         Each pub mod maps to one stdlib C file.
// ──────────────────────────────────────────────────────────────────────────
