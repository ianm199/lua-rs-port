//! Lua incremental tri-color mark-and-sweep GC. Phase D scope.
//!
//! This crate is permitted `unsafe` (ceiling: 20 blocks per
//! `harness/unsafe-budgets.toml`). Every block requires `// SAFETY: ...`.

// ──────────────────────────────────────────────────────────────────────────
// PORT STATUS
//   source:        (none — skeleton; Phase D populates from lgc.c)
//   target_crate:  lua-gc
//   confidence:    high
//   todos:         0
//   port_notes:    0
//   unsafe_blocks: 0
//   notes:         placeholder for incremental GC port
// ──────────────────────────────────────────────────────────────────────────
