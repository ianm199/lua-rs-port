# lua-rs-port

Safe-Rust port of Lua 5.4.7, driven by an AI-agent porting harness.

Two artifacts being built in parallel:

1. **`lua-rs`** — a behaviorally-equivalent pure-Rust implementation of Lua 5.4 that passes the official test suite in user mode (`_U=true`). Embeddable from Rust applications. WASM-compatible. **No C FFI dependencies.**
2. **The harness itself** — a reusable kit for AI-agent-driven C→Rust ports. Spec (`PORTING.md`), pre-computed analyses, oracle scripts, enforcement hooks, subagent roles. Published as a methodology, not just a port.

## Read in order

1. **`PORT_STRATEGY.md`** — what we're building, why, and the phase plan.
2. **`HARNESS_DESIGN.md`** — how we're driving the work with agents.
3. **`PORTING.md`** — the translation spec the agents read on every task.

## Quick start

Build the reference C Lua and run the baseline test suite (this is our oracle):

```bash
cd reference/lua-5.4.7
make macosx
cd ../lua-5.4.7-tests
../lua-5.4.7/src/lua -e "_U=true" all.lua
# should end with: final OK !!!
```

Run our Rust impl through the oracle on a specific test file:

```bash
./harness/oracle/run-test-file.sh constructs.lua
```

Run a full phase:

```bash
./harness/oracle/run-phase.sh B
cat harness/oracle/test-results.json
```

## Status

Pre-Phase-A setup. Reference C Lua built and verified. Harness scaffolding in place. `PORTING.md` and pre-computed analyses pending.

## Non-goals

- LuaJIT-level performance. We're matching reference Lua.
- Drop-in compat with C-Lua extensions (`LuaSocket`, `LFS`, etc.). Out of scope; possibly a future `lua-sys` shim.
- Drop-in for OpenResty / Neovim / WoW. Those use LuaJIT or 5.1, not 5.4.
