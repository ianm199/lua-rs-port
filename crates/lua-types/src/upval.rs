//! `UpVal` — closure upvalues. PORT_STRATEGY §3.8.

use crate::value::LuaValue;
use crate::StackIdx;
use std::cell::Cell;

/// A closure upvalue. Open upvalues point at a slot on a thread's stack
/// (referred to by index, since the stack reallocates). Closed upvalues
/// own the value.
///
/// State lives entirely in three `Cell` fields and is single-source-of-truth.
/// `open_thread_id` doubles as the open/closed discriminant: a non-negative
/// value is the owning thread id of an open upvalue; the [`CLOSED_TAG`]
/// sentinel (`-1`) means the upvalue is closed and its payload is in
/// `closed_value`. Valid thread ids are non-negative (the main thread is id 0),
/// so the sentinel is unambiguous. `open_idx` is the stack slot of an open
/// upvalue. Closing is terminal — there is no re-open path — so a `CLOSED_TAG`
/// tag never reverts.
///
/// Read the open shape with [`try_open_payload`](UpVal::try_open_payload)
/// (`None` once closed) and the closed payload with
/// [`closed_value`](UpVal::closed_value) / [`try_closed_value`](UpVal::try_closed_value).
/// The all-`Cell` layout lets `state.rs::upvalue_get` / `upvalue_set`
/// short-circuit the Open path with zero borrow-guard overhead, which is the
/// dominant cost in fibonacci-class recursion benchmarks.
#[derive(Debug)]
pub struct UpVal {
    open_thread_id: Cell<i64>,
    open_idx: Cell<u32>,
    closed_value: Cell<LuaValue>,
}

/// Sentinel placed in `open_thread_id` once the upvalue has been closed.
/// Valid thread ids are non-negative (the main thread is id 0), so -1 is
/// unambiguous.
const CLOSED_TAG: i64 = -1;

impl UpVal {
    pub fn open(thread_id: usize, idx: StackIdx) -> Self {
        UpVal {
            open_thread_id: Cell::new(thread_id as i64),
            open_idx: Cell::new(idx.0),
            closed_value: Cell::new(LuaValue::Nil),
        }
    }

    pub fn closed(v: LuaValue) -> Self {
        UpVal {
            open_thread_id: Cell::new(CLOSED_TAG),
            open_idx: Cell::new(0),
            closed_value: Cell::new(v),
        }
    }

    pub fn is_open(&self) -> bool {
        self.open_thread_id.get() >= 0
    }
    pub fn is_closed(&self) -> bool {
        self.open_thread_id.get() < 0
    }

    /// Zero-overhead read of the open shape used by `upvalue_get` /
    /// `upvalue_set` and every out-of-crate consumer that inspects an open
    /// upvalue's `(thread_id, idx)`. Returns `Some((thread_id, idx))` when the
    /// upvalue is still open, `None` once it has been closed.
    #[inline(always)]
    pub fn try_open_payload(&self) -> Option<(usize, StackIdx)> {
        let tid = self.open_thread_id.get();
        if tid < 0 {
            None
        } else {
            Some((tid as usize, StackIdx(self.open_idx.get())))
        }
    }

    /// Returns the closed-side value. Callers must have confirmed the
    /// upvalue is closed (`try_open_payload` returned `None`).
    #[inline(always)]
    pub fn closed_value(&self) -> LuaValue {
        self.closed_value.get()
    }

    pub fn close_with(&self, v: LuaValue) {
        self.open_thread_id.set(CLOSED_TAG);
        self.open_idx.set(0);
        self.closed_value.set(v);
    }

    pub fn set_closed_value(&self, v: LuaValue) {
        self.open_thread_id.set(CLOSED_TAG);
        self.open_idx.set(0);
        self.closed_value.set(v);
    }

    pub fn try_closed_value(&self) -> Option<LuaValue> {
        if self.is_closed() {
            Some(self.closed_value.get())
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn closed_scalar_write_updates_canonical_value() {
        let uv = UpVal::closed(LuaValue::Int(1));

        uv.set_closed_value(LuaValue::Int(2));

        assert_eq!(uv.closed_value(), LuaValue::Int(2));
        assert_eq!(uv.try_closed_value(), Some(LuaValue::Int(2)));
        assert!(uv.is_closed());
        assert_eq!(uv.try_open_payload(), None);
    }

    #[test]
    fn close_with_sets_cell_closed_state() {
        let uv = UpVal::open(7, StackIdx(3));
        assert_eq!(uv.try_open_payload(), Some((7, StackIdx(3))));

        uv.close_with(LuaValue::Bool(true));

        assert_eq!(uv.closed_value(), LuaValue::Bool(true));
        assert_eq!(uv.try_closed_value(), Some(LuaValue::Bool(true)));
        assert!(uv.is_closed());
        assert_eq!(uv.try_open_payload(), None);
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// PORT STATUS
//   source:        src/lfunc.h, src/lfunc.c (UpVal struct)
//   target_crate:  lua-types
//   confidence:    high
//   todos:         0
//   port_notes:    0
//   unsafe_blocks: 0
//   notes:         UpVal with Open/Closed state. C uses a TValue* that switches
//                  between stack-pointing (open) and embedded (closed) via union; we
//                  use three Cell fields as the single source of truth. open_thread_id
//                  is the open/closed discriminant (CLOSED_TAG = -1 means closed),
//                  open_idx is the open stack slot, closed_value holds the closed
//                  payload. Hot-path upvalue_get/_set read the Cells directly with no
//                  RefCell borrow guards. Read open shape via try_open_payload(),
//                  closed payload via closed_value()/try_closed_value().
// ──────────────────────────────────────────────────────────────────────────────
