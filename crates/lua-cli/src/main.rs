//! Standalone `lua-rs` interpreter — minimal entry point that exercises the
//! full pipeline: `new_state` → `open_libs` → `load_string` → `pcall_k`.
//!
//! This is intentionally minimal — its job is to surface which `todo!()`
//! stubs block real execution, NOT to be a complete Lua interpreter.
//!
//! Usage:
//!   lua-rs '<lua source>'
//! Examples:
//!   lua-rs 'print("hello")'
//!   lua-rs '1+1'

use std::panic::{catch_unwind, AssertUnwindSafe};
use std::process::ExitCode;

use lua_stdlib::auxlib::load_string;
use lua_stdlib::init::open_libs;
use lua_vm::api::pcall_k;
use lua_vm::state::new_state;

const MULTRET: i32 = -1;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        eprintln!("usage: {} '<lua source>'", args[0]);
        eprintln!("example: {} 'print(\"hello\")'", args[0]);
        return ExitCode::from(2);
    }
    let source = args[1].as_bytes().to_vec();

    eprintln!("[1/4] Creating LuaState...");
    let result = catch_unwind(AssertUnwindSafe(|| {
        let mut state = new_state().ok_or("new_state returned None")?;

        eprintln!("[2/4] Opening standard library...");
        open_libs(&mut state).map_err(|e| format!("open_libs failed: {:?}", e))?;

        eprintln!("[3/4] Loading source (parse + compile)...");
        let status = load_string(&mut state, &source)
            .map_err(|e| format!("load_string failed: {:?}", e))?;
        if status != 0 {
            return Err(format!("load_string returned non-zero status: {}", status));
        }

        eprintln!("[4/4] Executing chunk...");
        let final_status = pcall_k(&mut state, 0, MULTRET, 0, 0, None)
            .map_err(|e| format!("pcall_k failed: {:?}", e))?;

        Ok::<_, String>(final_status)
    }));

    match result {
        Ok(Ok(status)) => {
            eprintln!("[ok] execution completed, status={:?}", status);
            ExitCode::SUCCESS
        }
        Ok(Err(msg)) => {
            eprintln!("[err] {}", msg);
            ExitCode::from(1)
        }
        Err(panic) => {
            let msg = if let Some(s) = panic.downcast_ref::<String>() {
                s.clone()
            } else if let Some(s) = panic.downcast_ref::<&str>() {
                s.to_string()
            } else {
                "(non-string panic payload)".to_string()
            };
            eprintln!("[panic] {}", msg);
            ExitCode::from(101)
        }
    }
}

// ──────────────────────────────────────────────────────────────────────────
// PORT STATUS
//   source:        (minimal entrypoint; not a port of lua.c — that's Phase F)
//   target_crate:  lua-cli
//   confidence:    high
//   todos:         0
//   port_notes:    0
//   unsafe_blocks: 0
//   notes:         drives new_state → open_libs → load_string → pcall_k.
//                  Designed to surface the first todo!() panic on a hello-
//                  world program, not to be a complete interpreter.
// ──────────────────────────────────────────────────────────────────────────
