//! Lua coroutines via stackful context switching (`corosensei`). Phase E scope.
//!
//! This crate is permitted `unsafe` (ceiling: 10 blocks per
//! `harness/unsafe-budgets.toml`). Every block requires `// SAFETY: ...`.

// ──────────────────────────────────────────────────────────────────────────
// PORT STATUS
//   source:        (none — skeleton; Phase E populates from lcorolib.c)
//   target_crate:  lua-coro
//   confidence:    high
//   todos:         0
//   port_notes:    0
//   unsafe_blocks: 0
//   notes:         placeholder for stackful coroutine port
// ──────────────────────────────────────────────────────────────────────────
