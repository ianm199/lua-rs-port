# Phase G: Running LuaRocks against lua-rs

The PORT_STRATEGY.md §8 "this is real software" demo. After Phase F lands us at 95-98% upstream-test parity, this phase tackles the harder question: **can a real Lua application built with the Lua ecosystem run on our port?** LuaRocks is the canonical answer — it's pure Lua + a handful of C dependencies + an HTTP fetch + filesystem traversal + subprocess for build steps.

This doc captures the strategic choice (Rust-native modules vs. C ABI compat) and the concrete path to a working demo.

## Current verified status: LuaRocks works for pure-Lua rocks

LuaRocks 3.11.1 now runs far enough under `lua-rs` to install and use pure-Lua
rocks.

Verified commands:

```bash
RUSTFLAGS='-Awarnings' cargo build -q --bin lua-rs

curl -sSL https://luarocks.org/releases/luarocks-3.11.1.tar.gz | tar xz -C /tmp

mkdir -p /tmp/lua-rs-bin
ln -sf "$PWD/target/debug/lua-rs" /tmp/lua-rs-bin/lua
ln -sf "$PWD/target/debug/lua-rs" /tmp/lua-rs-bin/lua5.4

HOME=/tmp/lua-rs-luarocks-home \
PATH="/tmp/lua-rs-bin:$PATH" \
LUA_PATH="/tmp/luarocks-3.11.1/src/?.lua;/tmp/luarocks-3.11.1/src/?/init.lua" \
  ./target/debug/lua-rs /tmp/luarocks-3.11.1/src/bin/luarocks --tree /tmp/lua-rs-remote-tree install inspect
```

Confirmed behavior:

- `--version`, `help`, `config lua_version`, `path`, and `list` exit 0.
- Local `luarocks make` of a toy pure-Lua rock exits 0.
- Remote `luarocks search inspect` exits 0.
- Remote `luarocks install inspect` exits 0.
- The installed `inspect` module can be loaded and run by `lua-rs`.
- `luarocks show`, `which`, and `list` work against the installed tree.

Native C rocks remain outside the current compatibility boundary. A probe with
`luarocks install luafilesystem` reached the build step and failed at Lua header
detection:

```text
Error: Build error: Failed finding Lua header lua.h
```

That is expected for now. Even after headers exist, stock native rocks require a
real PUC-Rio Lua C API/ABI layer or per-module Rust-native replacements.

**What landed to make this work:**

- **Script arguments in `lua-cli`** — `lua-rs script.lua arg1 arg2` now
  populates global `arg` and passes `arg1`, `arg2`, ... as chunk varargs. This
  removed the misleading `dofile(...)` smoke harness.
- **Clean `os.exit` process control** — `os.exit` now uses a typed `LuaExit`
  panic payload so Lua `pcall` does not catch it as an ordinary error, and the
  CLI converts it to the process `ExitCode` without printing a panic.
- **`os.execute` hook registration** — LuaRocks probes candidate interpreters
  through `os.execute(cmd .. " > tmpfile")`; the CLI now installs the shell
  execution hook.
- **`lfs.lock_dir`** — LuaRocks locks install trees with
  `lfs.lock_dir(path):free()`. The Rust-native `lfs` module now implements the
  needed atomic `lockfile.lfs` path.
- **Directory read error propagation** — LuaRocks' macOS filesystem probe
  distinguishes directories from files by reading an opened directory and
  checking errno `21` (`EISDIR`). The file-handle trait now preserves read error
  errno/message so that probe works.
- **Shebang masking in `lua-cli`** — source files whose first byte is `#` can
  now be run directly, so the stock LuaRocks script no longer needs a
  `tail -n +2` copy.

**Important debugging correction:**

The earlier smoke:

```bash
lua-rs -e 'arg={[0]="luarocks","--version"}; dofile("/tmp/luarocks_noshebang.lua")'
```

was misleading. `dofile` does not forward varargs to the script, so LuaRocks did
not actually receive `--version`. The temporary wrapper form was:

```lua
local f = assert(loadfile("/tmp/luarocks_noshebang.lua"))
return f(table.unpack(arg, 1, #arg))
```

The normal path no longer needs that wrapper because `lua-cli` now handles
script arguments.

**Remaining known gaps:**

1. **Program name cosmetic** — LuaRocks help/version still prints `=[C]` in
   places where stock Lua would show the script name.
2. **Native C rocks** — header discovery and C API/ABI compatibility are not
   solved. Prefer Rust-native module ports for targeted rocks unless a full ABI
   effort is explicitly chosen.
3. **More package coverage** — `inspect` proves remote pure-Lua rock install,
   but a broader curated matrix should be added before public claims get
   stronger than "pure-Lua rocks can work."

---

## The strategic choice: C ABI compat vs. per-module Rust-native

### What is an ABI?

**API** is the source-level contract (function signatures, types — what you see in `lua.h`).
**ABI** (Application Binary Interface) is the *machine-level* contract: which CPU registers hold which arguments, how structs are laid out in memory, what symbol names are exported, how errors propagate.

Two pieces of compiled code can talk across a binary boundary (e.g., `dlopen("liblfs.so")` + `dlsym("luaopen_lfs")`) only if they agree at the ABI level — not just the API level.

### Why C-Lua gets a stable C ABI "for free"

1. **Every platform's default ABI is the C ABI.** gcc / clang emit machine code following the System V AMD64 (or Microsoft x64) calling convention by default. When something else does `dlsym(...)` and calls the function, it uses the same convention. No translation needed; the ABI is the machine's natural language.
2. **C is the lingua franca of system ABIs.** Every dynamic linker is specified in C terms. The infrastructure for binary plugins is C-shaped.
3. **Lua's API is *designed* to be ABI-stable.** `lua.h` exposes `lua_State` as an opaque type — you never see its fields. Everything is function calls (`lua_pushinteger(L, n)`), not field access. The author can reorganize internals between 5.4.0 and 5.4.7 without breaking any binary module. Compare with Python's C API, where some macros expose `PyObject->ob_refcnt` directly — much more brittle.

Lua's design choice (opaque `lua_State` + stack-based API) is what makes `lfs.so` from 2015 still loadable against `liblua-5.4.so` from 2023.

### Why our Rust port can't piggyback on this

Three reasons:

| | C-Lua | Our Rust port |
|---|---|---|
| Default calling convention | System C ABI by default | `extern "Rust"` by default; explicit `extern "C"` per fn |
| Default struct layout | C-standard (predictable padding/alignment) | `#[repr(Rust)]` — unspecified; need `#[repr(C)]` for every exposed type |
| Error propagation | longjmp/setjmp — native to C | `Result<T, E>` — incompatible with longjmp at the machine level |
| Symbol mangling | None | Mangled by default; `#[no_mangle]` per fn |
| Unsafe budget | All C is "unsafe" by Rust standards; just accepted | Every cross-boundary call is `unsafe` — accountable per block |

The longjmp/Result mismatch is the deepest problem. When a C module calls `luaL_error(L, "boom")`, in C-Lua that's a longjmp unwinding the C stack to the nearest `lua_pcall` setjmp. Rust frames in between are **unsafe to longjmp through** — destructors get skipped, leaking memory or leaving `RefCell`s borrowed forever.

### Cost comparison

| Approach | Cost | What you get |
|---|---|---|
| **C ABI compat** | ~8 weeks human + ~$2-5k agent | Every Lua C module ever compiled works (lfs, luasocket, lpeg, anything). True "drop-in replacement for liblua-5.4.so". |
| **Rust-native per module** | ~$30-50 + half-day per module | Only the modules you specifically port work. But each is small, safe, debuggable. |

The breakeven: if you need to install rocks like `luaposix` or `luasec` that don't have viable Rust-native equivalents AND don't want to write them, C ABI compat is unavoidable. For the LuaRocks-running-itself goal, the per-module path is dramatically cheaper.

## Recommended path: Rust-native modules

LuaRocks itself needs only ~4-5 C modules at runtime. Each is small. Port them as Rust-native modules loaded through the dynlib hook pattern we already shipped (`8c48cb1`).

### Module #1: `lfs-rs` (LuaFileSystem)

**The biggest dependency.** Used by literally every LuaRocks invocation for directory traversal, file attributes, path manipulation.

Required functions for LuaRocks (top 8 of lfs's ~14):

| lfs function | What it does | Rust equivalent |
|---|---|---|
| `lfs.attributes(path)` | stat — returns `{mode, size, modification, ...}` | `std::fs::metadata(path)` |
| `lfs.dir(path)` | iterator over directory entries | `std::fs::read_dir(path)` |
| `lfs.mkdir(path)` | create directory | `std::fs::create_dir(path)` |
| `lfs.rmdir(path)` | remove empty directory | `std::fs::remove_dir(path)` |
| `lfs.chdir(path)` | change cwd | `std::env::set_current_dir(path)` |
| `lfs.currentdir()` | get cwd | `std::env::current_dir()` |
| `lfs.touch(path, atime, mtime)` | set file times | `filetime` crate |
| `lfs.link(old, new, symlink)` | hard/symlink | `std::fs::hard_link` / `std::os::unix::fs::symlink` |
| `lfs.lock_dir(path)` | install-tree lock | atomic `lockfile.lfs` creation + `free()` |

Skip for now: `lfs.lock` / `lfs.unlock` (file-region locks; LuaRocks does not
use these for the current flow), `lfs.symlinkattributes` (rarely needed),
`lfs.setmode` (Windows-specific, no-op on Unix).

**Implementation shape**: new crate `crates/lua-rs-lfs/`. Mirror the `crates/lua-cli-test-rust-module/` skeleton from the dynlib slice. Each function is 5-30 LOC of `std::fs` wrapping.

**Non-trivial bits**:
1. `lfs.dir` returns an iterator backed by `read_dir`. Wrap in userdata + push a `lfs_dir_next` closure that uses the userdata as upvalue. ~50 LOC.
2. Mode-bit translation (`S_IFREG`, `S_IFDIR`, etc.) for `attributes.mode`. ~30 LOC of `cfg(unix)` / `cfg(windows)`.

**Effort**: 1 Opus run, $30-50, half-day. ~300-400 LOC total.

### Module #2: `os.execute` hook

LuaRocks uses `os.execute` to invoke `gcc`, `make`, `tar`, `unzip` during builds. Currently a stub returning `not implemented`.

**Implementation**: new hook on `GlobalState`:

```rust
pub type OsExecuteHook = fn(cmd: &[u8]) -> Result<i32, LuaError>;
```

Backed in `lua-cli` by `std::process::Command::new("sh").arg("-c").arg(...)`. Returns exit code mapped to Lua's `(boolean, "exit"|"signal", code)` tuple.

**Caveat for the demo**: real LuaRocks builds invoke compilers. We can either:
- Allow real subprocess execution (security tradeoff for demo purposes)
- Pre-install only "binary" rocks or pure-Lua rocks that skip the build step
- Sandbox via specific allowlist

**Effort**: $5-10, ~50 LOC. Same pattern as `file_remove_hook`.

### Module #3: `socket-rs` or `file://` repo

LuaRocks fetches rockspecs and tarballs over HTTPS. Two paths:

**Path A — Real HTTP via `ureq`** (~$30, ~200 LOC):
- New crate `crates/lua-rs-socket/` exposes a `socket.http.request(url)` shaped like luasocket's HTTP module
- Backed by `ureq` (blocking HTTP, bundles `rustls`) — small dep, no async runtime
- Sufficient for `luarocks install <rock>` against the real LuaRocks server

**Path B — file:// local repo** ($0 incremental, only LuaRocks config):
- Pre-mirror a small set of rocks to a local directory
- Point LuaRocks config at `file:///path/to/local-repo`
- Avoids networking entirely for the demo

Path B is the right call for the first demo. Path A is the right call for production. Both are tractable.

### Module #4: Crypto digests

LuaRocks verifies rock integrity via MD5/SHA256. Wire RustCrypto crates (`md-5`, `sha2`) as a tiny shim. ~$5, ~30 LOC.

### Module #5: Continuation support (Phase F-3.a)

`io.lines` and `file:lines` use coroutines internally to provide iterator-style file reading. LuaRocks reads many small files via these. Phase F-3.a (the continuation slice spec'd in `crates/lua-vm/src/api.rs:1772` TODO) covers this.

Already on the Phase F roadmap; mentioned here because LuaRocks specifically needs it.

## Full effort estimate

| Slice | Cost | Time |
|---|---|---|
| Finish Phase F (95-98% upstream) | $150 | 1 week |
| `lfs-rs` (LuaFileSystem in Rust) | $50 | half-day |
| `os.execute` hook | $10 | 1 hour |
| HTTP via `ureq` OR file:// repo | $30 / $0 | half-day / 0 |
| Crypto digests | $5 | 1 hour |
| LuaRocks integration testing + glue | $50 | 1 day |
| **Total beyond Phase F** | **~$145-175** | **2-3 days** |

**End state**: `target/debug/lua-rs path/to/luarocks install <pure-lua-rock>` succeeds against a local file:// repo. That's the §8 "Lua 5.4 in safe Rust runs LuaRocks" tagline made literal.

## What this path explicitly does NOT give you

- ❌ **Loading stock Lua C modules** like upstream `liblfs.so`, `libluasocket.so`. Each would need a Rust-native port OR the C ABI compat layer.
- ❌ **Rocks with C build dependencies** beyond what we've ported. `luaposix`, `luasec`, `lzlib` etc. each need their own Rust-native port or the C ABI.
- ❌ **Bytecode compatibility** — `.luac` files compiled by upstream `luac` won't load. Most users don't care.

These are tractable as follow-ups but each is its own slice.

## The strategic decision point

Two viable end-states for "real software" credibility:

1. **"Lua 5.4 in safe Rust runs LuaRocks against a curated rock set"** — what this doc plans. ~$170 + 2-3 days. Pure Rust, no unsafe budget expansion, demo runs.
2. **"Lua 5.4 in safe Rust is a literal drop-in replacement for liblua-5.4.so"** — the C ABI path. ~$5k + 8 weeks + major unsafe-budget expansion. Universal but expensive.

Recommendation: do (1) first as the headline demo. Reconsider (2) only if a specific use case requires loading a stock C module that doesn't have a Rust-native port.
