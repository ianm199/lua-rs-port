# Future Goals

This document separates the compatibility targets for `lua-rs`. They are often
collapsed into one phrase, but they are very different engineering projects.

## Current Target: Lua Source Compatibility

The current project target is Lua 5.4.7 source/runtime compatibility:

- parse and execute Lua source;
- implement Lua 5.4 value, table, closure, coroutine, metatable, error, and GC
  semantics;
- provide the Lua standard libraries through the Rust runtime;
- pass the upstream Lua 5.4.7 official test suite in the repo harness.

As of 2026-05-24, the harnessed official suite passes 44/44 tests.

That is strong evidence for Lua language compatibility. It is not the same as
being a drop-in replacement for PUC-Rio Lua's C API or binary ABI.

## Near-Term Goal: Rust-Native Embedding

The natural next public API target is a Rust-native embedding interface:

- create a Lua state from Rust;
- load source or bytecode;
- call Lua functions from Rust;
- expose Rust functions and user data to Lua;
- control resource limits and garbage collection;
- report errors without C `longjmp` semantics;
- keep the public API safe where possible and explicitly isolate unsafe internals.

This should be designed for Rust users first. It does not need to mimic `lua.h`
exactly to be useful.

## Why a Pure-Rust Lua for Embedding

The Rust-native embedding API is not just an ergonomics nicety. A pure-Rust Lua
has two clusters of advantages over today's C-backed bindings — bindings that
link PUC-Rio Lua and expose a Rust wrapper around the C interpreter.

### 1. Build and deployment

Linking C Lua drags the entire C build model along with it:

- The build needs a C toolchain, and cross-compilation inherits all of C's
  cross-compilation pain.
- A pure-Rust Lua is just a crate: `cargo build` everywhere, trivial
  cross-compilation, no `cc` or `make`, clean reproducible builds.
- The sharpest version is **WASM and embedded targets**. Pure Rust compiles to
  `wasm32` cleanly, whereas getting C Lua into a WASM module is genuinely
  painful. If the deployment target is unusual, that is a concrete win.

### 2. Safety and sandboxing of untrusted scripts

This is the real differentiator, and it is two distinct things.

**Memory safety of the implementation, not just the wrapper.** A C-backed
binding can give a safe *API*, but the interpreter underneath is still C — so a
bug in the Lua core is a memory-safety bug in the host process. A
mostly-safe-Rust VM means the *implementation* is memory-safe, not only the
wrapper around it.

**Real resource sandboxing.** C Lua lets you bolt on instruction hooks and a
custom allocator, but it is bolted on and sharp-edged. Crucially, C Lua's error
model is `longjmp`, and bridging `longjmp` with Rust's stack unwinding and
destructors is one of the hardest, most soundness-sensitive parts of any
Lua-in-Rust binding — existing bindings work hard to contain it. A pure-Rust Lua
built **stackless with a fuel system** gives, by construction:

- bounded CPU and memory;
- guaranteed return-to-caller (no runaway native frames);
- a native `Result` error model;
- no `longjmp` hazard at all.

For multi-tenant "run untrusted user scripts" workloads, that is a qualitative
difference, not a marginal one.

### Smaller wins that ride along

- A stackless design makes Lua-coroutine / Rust-async interleaving natural, where
  C Lua fights you.
- A native implementation can let Rust values participate in Lua's GC more
  seamlessly than a C binding's lifetime-juggling allows.

### Honest status

None of the sandboxing guarantees above exist yet. `lua-rs` today is a runtime
and CLI, not a hardened embedding sandbox, and its current incremental
mark-and-sweep GC is not the stackless + fuel design this argument assumes. This
section is the *destination* that justifies the Rust-native embedding API — not a
description of what ships today.

## Possible Long-Term Goal: C API Compatibility

C API compatibility would mean C code can embed `lua-rs` through functions shaped
like Lua 5.4's public API:

- `lua_newstate`, `lua_close`;
- stack operations such as `lua_gettop`, `lua_settop`, `lua_pushvalue`;
- loading and calling APIs such as `lua_load`, `lua_pcallk`, `lua_callk`;
- table/global/registry APIs such as `lua_getfield`, `lua_setfield`,
  `lua_rawgeti`, `luaL_ref`;
- userdata, metatable, finalizer, and uservalue support;
- `lauxlib.h` helpers such as `luaL_check*`, `luaL_error`, `luaL_Buffer`,
  `luaL_newmetatable`, and `luaL_requiref`;
- debug APIs such as `lua_getstack`, `lua_getinfo`, hooks, locals, and upvalues;
- allocator compatibility through `lua_Alloc`;
- C-facing headers and a linkable library artifact.

This is plausible as a compatibility layer, but it should be treated as a
separate subsystem. It would need its own C conformance tests, small embedding
programs, and native module fixtures.

## Hardest Target: ABI Drop-In Compatibility

ABI drop-in compatibility would mean existing C host programs or compiled Lua C
modules can link or load against `lua-rs` unchanged, as if it were `liblua`.

That requires more than exposing similarly named functions:

- exact exported symbol names and platform linker behavior;
- C-compatible type sizes and calling conventions for `lua_State`,
  `lua_CFunction`, `lua_KFunction`, `lua_Integer`, `lua_Number`, `lua_Debug`,
  `lua_Reader`, `lua_Writer`, and `lua_Alloc`;
- stack-index, pseudo-index, registry, upvalue, error, and continuation
  behavior matching PUC-Rio Lua closely enough for real C modules;
- support for arbitrary `.so`/`.dylib` Lua modules calling into the C API;
- userdata and finalization behavior that matches C module expectations;
- allocator behavior compatible with `lua_newstate`;
- a safe policy for PUC-Rio Lua's `setjmp`/`longjmp` style error unwinding.

The unwinding model is the largest safety and design issue. PUC-Rio Lua uses
long-jump based error propagation. Rust code cannot safely assume arbitrary C
`longjmp` through Rust frames, and Rust unwinding through C frames is also
constrained. Any serious ABI project needs an explicit boundary design before
implementation.

## Suggested Order

1. Keep source compatibility green with the official suite.
2. Design and stabilize a Rust-native embedding API.
3. Build a small C API compatibility crate as an experiment.
4. Add C fixture programs that cover stack operations, protected calls, registry
   references, userdata, finalizers, and callbacks.
5. Add simple native-module loading fixtures.
6. Decide whether `longjmp`-compatible behavior is feasible without compromising
   the safety model.
7. Only claim C API compatibility after the C fixture suite is broad and green.
8. Treat ABI drop-in compatibility as a separate release line unless proven
   practical.

## Public Claim Guidance

Good current phrasing:

> `lua-rs` is a Lua 5.4.7-compatible runtime implemented in Rust. The preview
> release targets Lua source/runtime compatibility first. Rust-native embedding
> and C API compatibility are future goals.

Avoid claiming:

- complete PUC-Rio Lua C API compatibility;
- ABI drop-in compatibility with `liblua`;
- compatibility with arbitrary existing Lua C modules;
- completely safe Rust.
