//! Lua value types, error types, and shared newtypes.
//!
//! See `PORT_STRATEGY.md` §3 for the design decisions.

/// Index into the Lua value stack. **Never a pointer or borrow.** Stack
/// reallocates; only indices are stable across mutations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct StackIdx(pub u32);

/// Index into the call-info stack.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CallInfoIdx(pub u32);

// ──────────────────────────────────────────────────────────────────────────
// PORT STATUS
//   source:        (none — new file, ground truth for design decisions)
//   target_crate:  lua-types
//   confidence:    high
//   todos:         0
//   port_notes:    0
//   unsafe_blocks: 0
//   notes:         skeleton; LuaValue, LuaError, GcRef to land Phase A
// ──────────────────────────────────────────────────────────────────────────
