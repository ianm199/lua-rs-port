# Execution goal: ship serde for omniLua, deep-spec async, open one PR

> This is the full execution spec referenced by the short `/goal`. Read it in
> full before starting, alongside `lua-rs-port/CLAUDE.md` and
> `crates/lua-rs-runtime/CLAUDE.md`. The oracle is the only truth-teller; a change
> that builds but no test/oracle has spoken on is unverified.
>
> Line numbers below are as of HEAD `d3e534ca` — grep to confirm, the file shifts.

## Phase 0 — Worktree (DO THIS FIRST)

Never work in the shared tree; one branch per worktree. Create a dedicated one off
the latest main and do ALL work there:

```bash
git -C lua-rs-port fetch origin
git -C lua-rs-port worktree add -b feat/embedding-serde ../lua-rs-port-serde origin/main
cd ../lua-rs-port-serde
```

The serde substrate (Value, IntoLua/FromLua, LossyIntPolicy, Table) is all on
main as of 0.3.7. If a needed piece is missing, rebase onto
`feat/embedding-hard-tier` and say so. All commits land on `feat/embedding-serde`.

Note: this goal file lives in the **main** tree (it is not committed and will not
appear in the new worktree). Read it by its absolute path; do your work in the
worktree.

## Phase 1 — Ship serde (the shippable feature)

Mirror `mlua`'s `LuaSerdeExt` so it reads as a drop-in for migrators.

### Public API (feature-gated behind a new optional `serde` feature)
- `pub trait LuaSerdeExt { fn to_value<T: Serialize>(&self, t: &T) -> Result<Value>; fn from_value<T: DeserializeOwned>(&self, v: Value) -> Result<T>; }` impl for `Lua`.
- Nice-to-have for JSON parity (do if cheap, else note as follow-up): `null()` sentinel + `array_metatable()` to force empty-table-as-array.

### Implementation
- New file `crates/lua-rs-runtime/src/serde_impl.rs`: a `serde::Serializer`
  (Rust→`Value`), a `Deserializer` (`Value`→Rust), and an error adapter
  (`serde::ser::Error + de::Error` → `omnilua::Error`). Add `#[cfg(feature="serde")]`
  module + the trait to `lib.rs`. It must carry a PORT STATUS trailer.
- Pure additive Rust layer — **no VM/GC/unsafe changes** (recon confirmed none are
  needed). If you reach for `unsafe`, stop: you've gone wrong.

### Substrate anchors (use these, don't re-derive)
- `Value` enum (note Integer vs Number are distinct): `lib.rs:1992`.
- `IntoLua`/`FromLua`: `lib.rs:3170`; existing `Option` `:3429`, `Vec` `:3453`, `HashMap` `:3483` impls are your pattern.
- Build: `create_table` `:1130`, `create_string(bytes)` `:1147`, `Table::set` `:2134`. Read: `Table::get` `:2118`, `raw_pairs` `:2207`, `LuaString::as_bytes` `:2365` / `to_str` `:2369`.
- **Int/float version seam is already solved** — route integers through
  `LossyIntPolicy` + `lower_host_int` (`lib.rs:4244`, used at the Integer arm
  `~:2043`). Do NOT reinvent it.
- **`marshal_value` (`lib.rs:4315`) is your traversal blueprint** — the recursive
  Value-tree walk. Reuse its shape.

### Convention decisions (DECIDED — don't reopen these)
- **seq vs map:** `serialize_seq`/tuple → array table (keys `1..n`);
  `serialize_map`/struct → string-keyed table. On `deserialize_any`, use the
  heuristic: contiguous `1..n` integer keys and nothing else → seq, else map.
  Honor serde type hints (`deserialize_seq`/`deserialize_map`) when given.
- **strings/bytes:** `serialize_str` and `serialize_bytes` both → `LuaString`
  (Lua strings are bytes). Deserialize serves bytes via `as_bytes`; `deserialize_str`
  validates UTF-8 via `to_str`. **Never use `String`/`&str` for the Lua-side data.**
- **nil/Option/unit:** `None`/unit → `Value::Nil`; `Nil` → `None`/unit.
- **enums:** externally tagged, matching mlua — unit variant → its name as a
  string; other variants → single-key map `{ "Variant": payload }`.
- **cycles:** serialize side needs NO cycle detection (serde data is a tree).
  Deserialize from a cyclic Lua table into a recursive Rust type is a documented
  caveat; add a recursion-depth guard if cheap.
- **unsupported:** `Function`/`Thread`/`UserData`/`LightUserData` → a clear serde error.

### Tests — `crates/lua-rs-runtime/tests/serde_integration.rs`
Round-trip: nested structs, externally-tagged enums, `Vec`, `HashMap`, `Option`,
tuples, byte strings, and a `serde_json::Value` ↔ Lua interop case. Add a
**version-seam test**: serialize an `i64` into a float-only 5.1 instance and assert
`LossyIntPolicy::{WidenLossy, ErrorOnInexact}` behave correctly. Add `serde`,
`serde_derive`, `serde_json` as dev-deps.

### Gates (climb the ladder; this is the PR gate at the top)
1. `cargo build -p omnilua --features serde`
2. `cargo test -p omnilua --features serde` (new + existing green)
3. **wasm — serde's strategic point**: `cargo build -p omnilua --features serde --target wasm32-unknown-unknown` must compile.
4. `cargo test -p omnilua --test multiversion_oracle` stays green.
5. PR gate: `harness/run_official_all.sh` green; `specs/oracle/check.sh` ×5.
6. Hooks satisfied: no inline `//` comments (doc-comments only), no fallback
   patterns, no `String`/`&str` for Lua data, unsafe-budget unchanged (0 new),
   PORT STATUS trailer on `serde_impl.rs`.

## Phase 2 — Deep spec for async (DESIGN ONLY, no implementation)

Write `specs/embedding/async-integration.md` matching the existing
`specs/embedding/*.md` style. This is correctness-sensitive (it crosses the
GC/coroutine/RefCell boundary — the project's highest-stakes zone), so it gets a
spec + adversarial review before any code. The spec must cover, with file:line
evidence:

- **API surface:** `Lua::create_async_function`, `Function::call_async`,
  `Chunk::eval_async`/`exec_async`.
- **Mechanism:** async host fn = native fn that yields a poll-token; a Rust driver
  loops resume → poll future → resume. Ground it in the EXISTING continuation
  machinery: `lua_yieldk` (`do_.rs:1417`), the `u_c_k` slot + `LuaKFunction`
  (`state.rs:829`), `lua_resume`/`resume_coroutine` (`do_.rs:1322`/`:1257`),
  `aux_resume` (`coro_lib.rs:291`). State clearly: this is a boundary integration,
  NOT a VM rewrite — the continuation substrate already exists.
- **The hard parts (be explicit, propose solutions):**
  1. `create_function` doesn't expose yield-from-callback yet — what to surface.
  2. The continuation is a bare `fn` ptr + `ctx: isize` — you can't store a Rust
     `Future` in it; design the side-table keyed by an opaque token.
  3. The `with_state` `RefCell` borrow (`lib.rs:1002`) is held for the whole
     closure; async must release it across `.await`. Specify the borrow lifetime.
  4. **GC rooting across `.await`** — the coroutine + captured args MUST stay
     rooted while the future is pending (`RootedValue` `lib.rs:1952`,
     `suspended_parent_stacks` `coro_lib.rs:453`). This is the soundness crux; a
     wrong rooting here = invisible UAF under generational GC. Write the soundness
     argument.
  5. `Lua` is `!Send` → single-threaded executor only (tokio `LocalSet`);
     executor-agnostic otherwise. State this constraint and that async does NOT
     help the wasm wedge.
- **Test plan** and an explicit **"open questions for codex-review"** section.

## Phase 3 — Review + PR

- Run `/codex-review` (read-only second opinion) on the **serde diff** and address
  real findings before opening the PR. Note in the PR that the **async spec** also
  needs codex-review before it gets implemented.
- One PR against `main`, branch `feat/embedding-serde`. Title:
  `feat(omnilua): serde integration (LuaSerdeExt) + async design spec`.
- PR body, two clearly separated sections: **Ships now** (serde — what/why/gates
  passed, with the wasm-compiles line called out) and **Design only** (async spec,
  for review, not implemented). Link the closed-issue context if useful and propose
  filing async + (if deferred) `null()`/`array_metatable()` as follow-up issues.
- Commit messages end with the repo's Co-Authored-By trailer; do not push tags.

## Definition of done
serde merged-ready and green on all gates incl. wasm + multiversion + official
suite; `specs/embedding/async-integration.md` written and grounded in real
file:line anchors with a soundness argument; codex-review run on the serde diff;
one PR open against main with the two-section body. Report what shipped, what's
spec-only, and any gate you couldn't run.
