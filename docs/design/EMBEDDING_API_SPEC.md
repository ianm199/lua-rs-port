# Embedding API — implementation spec

Handoff spec for the implementation agent. Rationale and design tradeoffs live in
[EMBEDDING_API.md](EMBEDDING_API.md); this document is the build plan: phases,
concrete types, integration points in the current codebase, soundness
invariants, and acceptance criteria.

Goal: a Rust embedding API for lua-rs, mlua-shaped at the handle /
`create_function` / userdata layers, so the bms backend becomes a port of its
existing mlua backend rather than a rewrite. Build order targets the
bms → nano9 path: substrate first, conversion sugar last.

## Current state (verified against the tree)

- **Embedding entry point:** `lua-rs-runtime` (`LuaRuntime::new/with_hooks`,
  `exec(&mut self, src, name)`, `state()/state_mut()`, `HostHooks` builder).
- **Rust functions today:** `lua_CFunction = fn(&mut LuaState) -> Result<usize, LuaError>`
  (a **bare fn pointer**, stack-protocol: args on the Lua stack, return the
  pushed-result count). Stored in `GlobalState.c_functions: Vec<lua_CFunction>`,
  referenced by index, deduped by `fn_addr_eq`. `push_c_closure(f, n)` exists but
  `f` is still a bare fn. → **no captured state, no re-entrant context.**
- **GC:** `lua-gc::Heap::full_collect(roots)`; `trait Trace { fn trace(&self, &mut Marker) }`;
  roots are gathered by `GlobalState::trace` (`crates/lua-vm/src/trace_impls.rs`),
  which already traces `l_registry`, globals, threads, `to_be_finalized`, etc.
- **Registry:** `GlobalState.l_registry: LuaValue` with `LUA_RIDX_MAINTHREAD=1`,
  `LUA_RIDX_GLOBALS=2`. There is **no `luaL_ref`/`luaL_unref`** yet. The registry
  is already traced as a root — it is the anchor for the external root set.

The two limitations to remove: bare-fn (no capture) and `&mut self` (no
re-entrancy). Everything below follows.

## Phases and acceptance criteria

Each phase must keep the official suite at **44/44** (run from a clean worktree
against the built binary, per the project's verification pattern) and keep the
benchmark geomean unchanged (proof the hot loop is untouched).

### Phase 0 — shared, re-entrant state

Replace the `&mut self` access model with a shared handle that supports
re-entrancy, **without** putting a borrow/cell in the per-instruction dispatch
loop.

- Introduce `Lua` (or evolve `LuaRuntime`) as a cheap-clone, `!Send` handle whose
  state lives behind interior mutability. The VM borrows the state **once at a
  call boundary** and runs `execute()` with direct access; only the re-entrant
  callback boundary pays the cell cost.
- A callback receives a context (`&Lua` / a `Context`) it can re-enter through.

Acceptance: a registered function can call back into the VM during a `pcall`
without aliasing; `cargo bench` geomean unchanged vs baseline; 44/44.

### Phase 1 — GC external root set + owned handles

- Add an **external root set** anchored in `l_registry` (a `luaL_ref`-style
  integer-keyed ref array, or a dedicated root slab on `GlobalState`). Trace it
  from `GlobalState::trace` so rooted values are marked.
- Implement ref/unref: rooting returns a key; unref frees it. Ref-count or
  unique-key per handle.
- Owned handle types, each holding a root key + a back-reference to `Lua`:
  `Value`, `Table`, `Function`, `LuaString` (add `Thread`, `AnyUserData` later).
  `Clone` re-roots (or bumps refcount); `Drop` unroots exactly once.

Acceptance: a Rust-held `Table` survives `collectgarbage("collect")` and is still
usable; handles drop without leaking root slots; **Miri-clean** on the
handle/root-set unit tests; 44/44.

### Phase 2 — `create_function` with captured state (the gate)

- Extend the C-function mechanism to store **boxed closures** alongside bare fns:
  `Rc<dyn Fn(&Lua, A) -> Result<R>>` (and a `_mut` variant via interior
  mutability). The existing `c_functions: Vec<lua_CFunction>` becomes a registry
  of callables that can be either a bare fn or a boxed closure.
- Invocation: marshal Lua-stack args → closure inputs, run the closure with the
  re-entrant `Lua` from Phase 0, marshal the result back to the stack, propagate
  `Err` as a Lua error. Captured Lua handles (Phase 1) keep their referents
  rooted for the closure's lifetime.

Acceptance: register a stateful closure (captures an `Arc`/counter and a stored
`Function`), call it from Lua, have it call back into Lua and return a value;
the **bms Gate-2 function-registry bridge** compiles against this; 44/44.

### Phase 3 — userdata + metamethods (required for bms, not deferred)

- `AnyUserData` handle (Phase 1 anchored) wrapping a Rust value, with runtime
  borrow tracking (RefCell-style) to bridge Rust aliasing vs Lua re-entrancy.
- `UserData` trait with `add_method`/`add_method_mut`/`add_meta_method` (mirror
  mlua's `UserDataMethods`), backed by Phase-2 closures. Support
  `__index`/`__newindex`/arithmetic metamethods.

Acceptance: a Rust struct exposed to Lua with `__index`/`__newindex`; the **bms
reflection bridge** (`reference.rs`, ~460 LoC) ports from its mlua version with
mechanical changes only; 44/44.

### Phase 4 — conversion sugar (trails)

`FromLua`/`IntoLua` + `FromLuaMulti`/`IntoLuaMulti` with blanket impls
(`i64/f64/bool/String/&str/&[u8]/Option<T>/Vec<T>/HashMap/tuples`), later a
derive. Serves the direct-embedder profile; bms marshals via its own
`ScriptValue` and doesn't need it.

## Target API (mlua-shaped — mirror these names)

```rust
// state
let lua = Lua::new();                 // !Send, cheap clone
let t: Table = lua.create_table()?;
let s: LuaString = lua.create_string("x")?;

// run
lua.load(src).set_name(name).exec()?;
let v: i64 = lua.load("return 2+3").eval()?;     // eval needs FromLua (Phase 4)
let g: Table = lua.globals();

// handles (owned, anchored)
t.set("k", v)?; let x: Value = t.get("k")?; let n = t.len()?;
let f: Function = t.get("fn")?; let r: Value = f.call(args)?;

// create_function (Phase 2)
let f = lua.create_function(|lua: &Lua, args: A| -> Result<R> { ... })?;
let f = lua.create_function_mut(|lua, args| { ... })?;       // FnMut

// userdata (Phase 3)
impl UserData for T {
    fn add_methods<M: UserDataMethods<Self>>(m: &mut M) { ... }
    fn add_meta_methods<M: UserDataMethods<Self>>(m: &mut M) { ... }
}
let ud: AnyUserData = lua.create_userdata(value)?;
```

Error type: `lua_rs::Error` (RuntimeError(Value), conversion errors, borrow
errors). Callbacks return `Result<_, Error>`; Rust panics must never cross the
boundary; Lua errors are values.

## Integration points (where to touch)

- Root set + ref/unref: new structure on `GlobalState`, anchored at/in
  `l_registry`; mark it in `GlobalState::trace` (`lua-vm/src/trace_impls.rs`).
- Closure storage + dispatch: extend `GlobalState.c_functions` and the
  C-function call path (`state_stub.rs` / `api.rs` `push_c_*` and the precall-C
  path in `lua-vm`). Verify exact signatures before editing.
- Re-entrant access: the `LuaState` borrow model in `lua-vm` (`execute()` is
  driven with `&mut`); introduce the cell at the call boundary only.
- Public surface: `lua-rs-runtime` (evolve `LuaRuntime`/add `Lua`); keep
  `HostHooks` as the capability layer.

## Soundness invariants (the part to get right)

State these as tested properties:

1. **Rooting:** every value referenced by a live handle is in the root set and is
   marked; collection can never free a value a live handle points to.
2. **Drop discipline:** a handle's `Drop` unroots exactly once; no double-unroot,
   no leaked root slot; refcount (if used) hits zero exactly when the last clone
   drops.
3. **GC-during-callback:** a callback may allocate, which may trigger collection;
   anything the callback holds (args, created values, captured handles) must be
   rooted for the duration. No "transient unrooted value held across an alloc."
4. **Re-entrancy aliasing:** entering the VM re-entrantly must not create two
   live `&mut` paths into the heap. The cell is borrowed at the boundary and
   released before re-entry; the dispatch loop never holds it across a callback.
5. **Generational write barrier:** rooting an old-gen object that then points to
   a young-gen object requires the existing barrier; verify the root set plays
   with generational mode.

## Anti-requirements

- No `'lua` lifetimes on handles (rlua's mistake). Handles are owned/anchored.
- No cell/borrow in the per-instruction dispatch loop (perf regression).
- Public API stays safe; new `unsafe` is confined to the anchor/Drop/GC-interface
  core and budgeted in `harness/unsafe-budgets.toml`. (Note: this work will
  *grow* the unsafe surface — a conscious call against the full-safety goal.)
- Do not regress the official suite or the benchmark geomean.
- Mirror mlua's public names where they exist.

## Verification

- Official suite 44/44 after each phase (clean worktree, `LUA_RS_BIN` against the
  built binary).
- Benchmarks: geomean unchanged vs the pre-work baseline (proves boundary-only
  cost; the dashboard has the baseline).
- Miri on handle/root-set/create_function unit tests.
- A **GC-torture test**: force a full collection between every handle op and
  every callback step; rooted handles must survive, dropped ones must free.
- A stress/fuzz test: random create/clone/drop of handles interleaved with
  collection and re-entrant callbacks; assert no leak, no UB (under Miri).

## mlua-shape mapping (so the bms backend ports, not rewrites)

| lua-rs | mlua |
|---|---|
| `Lua` | `mlua::Lua` |
| `Value` | `mlua::Value` |
| `Table` / `Function` / `LuaString` / `AnyUserData` | same names |
| `lua.create_function(...)` | `Lua::create_function` |
| `lua.create_table()` / `create_userdata()` | same |
| `UserData` + `UserDataMethods` (`add_method`, `add_meta_method`) | same |
| `FromLua`/`IntoLua` (+ `*Multi`) | same |
| `lua_rs::Error` / `Result` | `mlua::Error` / `mlua::Result` |

Keeping these aligned is what turns bms's `reference.rs` and the `FromLua`/
`UserData` impls for `ScriptValue` into a near-mechanical translation.

## Tradeoffs (carry into implementation)

- Costs are at the Rust↔Lua boundary (handle root/unroot, marshalling, dynamic
  dispatch), not the interpreter hot loop — pure-Lua perf is unaffected if
  invariant #4 holds. Mitigate boundary churn with scoped/transient handles
  (mlua's `scope`) later if needed.
- The real risk is soundness (invariants 1–5), not throughput. It's one-shot,
  high-stakes; lean on Miri + the torture test.
- It grows the audited `unsafe` surface — a deliberate tension with "get to full
  safety." Make that call explicitly and document each block with `// SAFETY:`.
