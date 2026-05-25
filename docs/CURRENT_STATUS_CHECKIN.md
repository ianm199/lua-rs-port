# Current Status Check-In

This is a quick handoff note for cleaning up public-facing docs without losing
the actual project state. It is intentionally plain; polish the root README from
these facts, not from old agent-worklog wording.

## Project State

`lua-rs` is a Lua 5.4.7 runtime implemented in Rust.

The strongest current facts:

- Published preview release exists on crates.io at `0.0.1`.
- Install command is `cargo install lua-cli`.
- Installed binary is `lua-rs`.
- `lua-rs -e 'print("hello")'` works.
- `lua-rs script.lua` works for basic script execution.
- The upstream Lua 5.4.7 official test suite passes 44/44 in the repo harness.
- Normal script execution has no dependency on the C Lua runtime.
- Most crates forbid `unsafe`; remaining unsafe is isolated and budgeted.
- Performance dashboard exists and is one of the best credibility artifacts.

Good one-line positioning:

> `lua-rs` is a Lua 5.4.7 runtime implemented in Rust, with a published preview
> CLI and a repo harness that passes the upstream Lua official test suite.

## Do Not Overclaim

Do not say this is:

- LuaJIT;
- a complete replacement for `liblua`;
- ABI-compatible with the PUC-Rio Lua C API;
- able to load arbitrary existing Lua C modules unchanged;
- completely safe Rust;
- a finished production runtime.

Also do not claim these CLI features yet:

- REPL;
- stdin execution;
- polished `--help`;
- complete `lua` command-line compatibility.

The CLI now has normal Lua-style script argument handling in the active
uncommitted work:

```bash
lua-rs script.lua arg1 arg2
```

This populates global `arg` and passes script args as chunk varargs. That was a
required LuaRocks blocker because LuaRocks reads command arguments from script
varargs.

## Public README Shape

Suggested README flow:

1. What it is.
2. Install from crates.io.
3. Basic usage.
4. Compatibility and conformance.
5. Performance dashboard.
6. Safety model.
7. Known limitations.
8. Crate layout.
9. Development and verification commands.

The harness/debugging story is valuable, but it should come after the runtime
story. The current README reads too much like an agent-porting log.

## Conformance

Public wording should distinguish source/runtime compatibility from C ABI
compatibility:

> The repository's Lua 5.4.7 official-test harness currently passes 44/44 tests.
> This is strong evidence for Lua source/runtime compatibility. It does not imply
> PUC-Rio Lua C API or ABI compatibility.

Useful verification commands:

```bash
RUSTFLAGS='-Awarnings' cargo build -q --bin lua-rs
RUSTFLAGS='-Awarnings' TEST_TIMEOUT_S=90 ./harness/run_official_all.sh
```

Related docs:

- `docs/OFFICIAL_TEST_INVESTIGATIONS.md`
- `docs/FUTURE_GOALS.md`

## Performance

The dashboard should be linked prominently once the public URL is stable.

Local dashboard:

```text
harness/bench/history/index.html
```

Dashboard source data:

```text
harness/evidence/ledger.jsonl
```

Builder:

```bash
python3 harness/bench/history.py
```

Important framing:

- Lower ratios are better.
- `1.00x` is parity with upstream PUC-Rio Lua on the same benchmark.
- Do not summarize the result as "faster than C"; the workload-level chart is
  the honest story.
- The dashboard styling in `harness/bench/history/index.html` is the version to
  preserve if another agent is making a shareable page.

Related docs:

- `docs/PERFORMANCE_PRINCIPLES.md`
- `docs/MATCHING_C_PERFORMANCE.md`
- `harness/bench/README.md`

## Safety

Good public wording:

> `lua-rs` exposes a safe public surface over a small audited unsafe core. Most
> crates forbid unsafe code. The remaining unsafe surface is budgeted in
> `lua-gc`, the `lua-cli` dynamic-loading backend, and the dedicated WASM ABI
> crates.

Do not call the project completely safe Rust.

Related docs:

- `docs/PUBLISH_READINESS.md`
- `docs/LUA_SYSTEM_DEEP_DIVE.md`

## Publishing

Crates.io publishing has been completed for the preview release. The public
crate install path is:

```bash
cargo install lua-cli
```

Published crates at `0.0.1`:

- `lua-gc`
- `lua-types`
- `lua-vm`
- `lua-code`
- `lua-lex`
- `lua-stdlib`
- `lua-coro`
- `lua-rs-lfs`
- `lua-parse`
- `lua-cli`

`lua-cli-test-rust-module` is intentionally unpublished.

## LuaRocks Status

LuaRocks is now working for a meaningful pure-Lua package path, but it is not
ready for native C rocks or a broad ecosystem headline.

Most important current finding:

- The old `dofile("/tmp/luarocks_noshebang.lua")` smoke was misleading because
  `dofile` does not forward varargs to the loaded script.
- The correct smoke shape is `loadfile(...)` followed by an explicit call with
  `table.unpack(arg, 1, #arg)`.

Verified current state after the current LuaRocks work:

- LuaRocks 3.11.1 boots under `lua-rs`.
- `--version` exits 0.
- `help` exits 0.
- `config lua_version` exits 0 and prints `5.4`.
- `path` exits 0 when `/tmp/lua-rs-bin/lua5.4` points at `target/debug/lua-rs`.
- `list`, `show`, and `which` work against an installed tree.
- Local `luarocks make` of a toy pure-Lua rock exits 0.
- Remote `luarocks search inspect` exits 0.
- Remote `luarocks install inspect` exits 0.
- The installed `inspect` module can be loaded and run under `lua-rs`.
- The stock LuaRocks script can be run directly; the CLI now masks a leading
  Unix shebang line before parsing.
- A native C rock probe with `luarocks install luafilesystem` reaches the build
  step and fails at missing `lua.h`, as expected for the current no-C-ABI
  boundary.

Current uncommitted implementation work:

- Added typed `LuaExit(i32)` payload in `lua-types`.
- Changed `os.exit` to use typed panic control flow so `pcall` does not catch it.
- Changed `lua-cli` to intercept `LuaExit` at the process boundary and suppress
  the default panic message for that payload only.
- Added script-argument handling in `lua-cli`.
- Registered the existing `os.execute` hook from `lua-cli`.
- Added `lfs.lock_dir` to the Rust-native `lua-rs-lfs` module.
- Preserved file read errno/message through `LuaFileHandle` so LuaRocks'
  macOS directory probe can distinguish directories from files.
- Added CLI shebang masking for script files whose first byte is `#`.

Direct checks already passed:

```bash
RUSTFLAGS='-Awarnings' cargo build -q --bin lua-rs
./target/debug/lua-rs -e 'os.exit(0, true)'
./target/debug/lua-rs -e 'os.exit(20, true)'
./target/debug/lua-rs -e 'pcall(os.exit, 7, true); print("after")'
./target/debug/lua-rs /tmp/lua-rs-args.lua alpha beta
```

LuaRocks smoke commands:

```bash
mkdir -p /tmp/lua-rs-bin
ln -sf "$PWD/target/debug/lua-rs" /tmp/lua-rs-bin/lua
ln -sf "$PWD/target/debug/lua-rs" /tmp/lua-rs-bin/lua5.4

HOME=/tmp/lua-rs-luarocks-home \
PATH="/tmp/lua-rs-bin:$PATH" \
LUA_PATH="/tmp/luarocks-3.11.1/src/?.lua;/tmp/luarocks-3.11.1/src/?/init.lua" \
  ./target/debug/lua-rs /tmp/luarocks-3.11.1/src/bin/luarocks --tree /tmp/lua-rs-remote-tree install inspect
```

Remaining LuaRocks work:

- fix the cosmetic `=[C]` program name in LuaRocks help/version output;
- add a curated pure-Lua package matrix before making broader public claims;
- keep native C rocks out of scope until either Rust-native module ports or a
  PUC-Rio C API/ABI layer exists.

The LuaRocks section in `docs/PUBLIC_README_HANDOFF.md` has been updated with
this status.

## Dirty Worktree Notes

Ignore the many dirty `.claude/worktrees/*` gitlink entries unless explicitly
cleaning agent worktrees. The relevant active source edits are:

- `crates/lua-types/src/error.rs`
- `crates/lua-types/src/filehandle.rs`
- `crates/lua-types/src/lib.rs`
- `crates/lua-stdlib/src/os_lib.rs`
- `crates/lua-stdlib/src/io_lib.rs`
- `crates/lua-rs-lfs/src/lib.rs`
- `crates/lua-cli/src/main.rs`

The new status doc itself is:

- `docs/CURRENT_STATUS_CHECKIN.md`

## Best Next Steps

For the README/presentation agent:

- rewrite root README around the runtime, not the harness;
- preserve performance dashboard styling when making it shareable;
- link conformance/performance/safety docs rather than copying all detail;
- keep LuaRocks as "in progress" unless the install path is actually verified.

For the next engineering pass:

- add a checked-in LuaRocks smoke script or harness target;
- fix the `=[C]` program-name/chunk-name cosmetic issue;
- broaden the pure-Lua package matrix;
- update `docs/PHASE_G_LUAROCKS_PLAN.md` with the new evidence;
- run the official suite before committing the CLI/runtime changes.
