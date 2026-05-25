# lua-rs

**Lua 5.4.7, reimplemented in safe Rust.**

`lua-rs` is a from-scratch Rust port of the reference [PUC-Rio Lua 5.4.7](https://www.lua.org/)
interpreter. It runs ordinary Lua programs with no C runtime dependency and
passes **44 / 44** of the upstream Lua test suite — the same `.lua` files the C
implementation is validated against.

[![CI](https://github.com/ianm199/lua-rs/actions/workflows/ci.yml/badge.svg)](https://github.com/ianm199/lua-rs/actions/workflows/ci.yml)
[![crates.io](https://img.shields.io/crates/v/lua-cli.svg?label=crates.io%2Flua-cli)](https://crates.io/crates/lua-cli)
[![license](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
[![upstream tests](https://img.shields.io/badge/upstream%20suite-44%2F44-0f8f68.svg)](#conformance)
[![performance](https://img.shields.io/badge/perf-live%20dashboard-2f6fed.svg)](https://ianm199.github.io/lua-rs/harness/bench/history/)

```bash
cargo install lua-cli       # crate: lua-cli  →  binary: lua-rs
lua-rs -e 'print("hello from lua-rs")'
```

> [!NOTE]
> The crates.io package is named **`lua-cli`**; it installs a binary named
> **`lua-rs`**. `cargo install lua-cli` is the install; `lua-rs` is what you run.

---

## Highlights

- **Passes the real Lua test suite** — the upstream PUC-Rio 5.4.7 `testes/` suite
  runs against this binary, 44/44. Not a subset, not a lookalike.
- **Safe Rust by default.** Most crates compile under `#![forbid(unsafe_code)]`;
  the only `unsafe` is a small, audited, budgeted core (GC + dynamic loader).
- **No C runtime.** A `.lua` script links no `liblua` and shells out to no C
  interpreter — it's a standalone Rust binary.
- **Runs LuaRocks.** Drives the real LuaRocks 3.11.1 to `search`, `install`,
  `show`, and `require` pure-Lua rocks like `inspect` — a real ecosystem
  workflow, not just conformance. (Native C rocks: not yet.)
- **Competitive performance, tracked publicly** — within ~1.3× of reference C on
  wall-time geomean, faster on some workloads, every commit
  [plotted live](https://ianm199.github.io/lua-rs/harness/bench/history/).
- **Built by an AI porting harness** — ~28k lines of C became safe Rust under a
  test-oracle-gated, multi-agent harness. See [How it was built](#how-it-was-built).

## Install

```bash
cargo install lua-cli        # installs the `lua-rs` binary into ~/.cargo/bin
```

Or from source:

```bash
git clone https://github.com/ianm199/lua-rs && cd lua-rs
cargo build --release --bin lua-rs
```

## Usage

```bash
lua-rs                            # interactive REPL
lua-rs script.lua                 # run a source file
lua-rs -e 'print(1 + 2)'          # run a one-liner
echo 'print("hi")' | lua-rs -     # read from stdin
lua-rs -v                         # version
```

It mirrors the standard `lua` CLI: a bare argument is a script filename, `-e`
runs a chunk, `-` reads stdin, and no arguments on a terminal opens the REPL.

## Conformance

The strongest claim here. The unmodified upstream Lua 5.4.7 test suite runs
against the `lua-rs` binary through a behavioral oracle (diff stdout + exit code
against reference C):

```bash
TEST_TIMEOUT_S=90 ./harness/run_official_all.sh   # → 44/44 PASS
```

Strong evidence for Lua source/runtime compatibility — it does *not* imply C
API/ABI compatibility. Per-test history:
[docs/OFFICIAL_TEST_INVESTIGATIONS.md](docs/OFFICIAL_TEST_INVESTIGATIONS.md).

## Performance

A dedicated **benchmark suite** — eight workloads (`fibonacci`, `mandelbrot`,
`binarytrees`, `closure_ops`, `table_ops`, `table_ops_long`, `string_ops`,
`string_ops_long`) — runs against reference PUC-Rio Lua 5.4.7 via
`harness/bench/compare.sh`. This is separate from the
[44/44 conformance suite](#conformance): it measures speed and memory, not
correctness.

### → [**Live performance dashboard**](https://ianm199.github.io/lua-rs/harness/bench/history/)

Each point is the ratio of `lua-rs` to reference lua-c on the same workload —
**lower is better; `1.00×` is parity with C.** At the latest benchmarked commit:

| Metric | Value | Reading |
|---|---|---|
| Wall-time geomean | **1.27×** | ~27% slower than C on average |
| RSS geomean | **1.96×** | ~2× the memory of C |
| Best workload (`table_ops_long`) | **0.38×** | faster than C |
| Worst workload (`binarytrees`) | **2.07×** | slowest relative workload |

This is **not** "faster than C" — it's a memory-safe reimplementation that's
*competitive* with C, with the full per-workload trajectory published rather
than reduced to one number. Method:
[docs/PERFORMANCE_PRINCIPLES.md](docs/PERFORMANCE_PRINCIPLES.md).

## Safety model

`lua-rs` is a mostly-safe runtime around a small, explicit unsafe kernel —
**not** "completely safe Rust," but not unsafe-everywhere either. The VM, parser,
lexer, bytecode compiler, standard library, and coroutine layer are all budgeted
at **zero** unsafe; per-crate ceilings are enforced by
`.claude/hooks/unsafe-budget.sh`. All real `unsafe` lives in two places:

- **GC core — `crates/lua-gc` (13 sites):** raw-pointer object identity, heap
  walking, gray-list traversal, sweep cursors, `Box::from_raw`. Soundness rests
  on one invariant — collection only runs at safepoints where every live
  `Gc<T>` is reachable through the traced root graph. This is the trusted kernel.
- **Dynamic loading — `crates/lua-cli` (5 sites):** `libloading` opening symbols
  from shared objects; narrow, only active for dynamic modules, each block
  `// SAFETY:`-justified.

It's a deliberate trade-off: `lua-rs` favors ecosystem/CLI compatibility (full
CLI, `require`/`package`, LuaRocks, dynamic modules). A purely sandboxed
embedding runtime can be safer still — which is exactly the
[long-term embedding goal](#roadmap) below. Details:
[docs/LUA_SYSTEM_DEEP_DIVE.md](docs/LUA_SYSTEM_DEEP_DIVE.md),
[docs/PUBLISH_READINESS.md](docs/PUBLISH_READINESS.md).

## How it was built

The runtime is the artifact; the **AI-agent porting harness** is the method — and
the more reusable result. ~28k lines of C became safe Rust via bounded,
single-purpose agents (translator, compiler-fixer, test-fixer, read-only
verifier) gated by a non-negotiable **oracle**: a change is unverified until the
test suite or a structural diff matches reference C. Guardrails (unsafe budgets,
forbidden-pattern bans, required status trailers, a verify-gate) are enforced as
hooks — and the read-only verifier *cannot* mark a test passing, anti-sycophancy
by construction. See [PORTING.md](PORTING.md), [HARNESS_DESIGN.md](HARNESS_DESIGN.md),
and [docs/RETROSPECTIVE_AND_PRODUCTIZATION.md](docs/RETROSPECTIVE_AND_PRODUCTIZATION.md).

## Roadmap

- **Embeddable pure-Rust Lua — the long-term goal.** Let Rust projects embed Lua
  scripting with no C toolchain or C ABI. Because it's just a crate, you get
  `cargo build` everywhere, trivial cross-compilation, and clean `wasm32` /
  embedded targets — no `cc` or `make`, and none of C Lua's WASM pain. And
  because the VM itself is (mostly) safe Rust, the aim is **safer sandboxing of
  untrusted scripts** than today's C-backed bindings: a memory-safe
  *implementation*, not just a safe wrapper, plus a stackless + fuel design for
  bounded CPU/memory, guaranteed return-to-caller, a native `Result` error model,
  and no `longjmp`. See [docs/FUTURE_GOALS.md](docs/FUTURE_GOALS.md).
- **Performance parity with PUC-Rio Lua** — close the ~1.27× wall-time gap,
  tracked commit by commit on the
  [dashboard](https://ianm199.github.io/lua-rs/harness/bench/history/).
- **A testbed for runtime research** — prototype new garbage-collection
  strategies and other language/runtime features against a real conformance +
  benchmark harness, validated by the oracle rather than by intuition.
- **LuaRocks: native C rocks** — packages with C extensions, via a Lua C API/ABI
  layer or Rust-native module replacements. Pure-Lua rocks already work.

## Limitations and non-goals

- Not LuaJIT, and not targeting LuaJIT-level performance.
- Not a C-ABI drop-in — stock Lua C modules expecting `liblua` won't load unchanged.
- Not for Lua 5.1 ecosystems (OpenResty, Neovim's LuaJIT embedding, WoW addons) —
  this is Lua 5.4.
- Native C rocks aren't supported yet — only pure-Lua rocks.

## Project layout

```
crates/
  lua-lex, lua-parse, lua-code   # front end: lexer, parser, bytecode compiler
  lua-vm                         # the register VM and core runtime
  lua-types                      # LuaValue, tables, strings, errors
  lua-gc                         # garbage collector (budgeted unsafe)
  lua-stdlib                     # standard library
  lua-coro                       # coroutines
  lua-cli                        # the `lua-rs` binary + dynamic-load backend
harness/                         # the porting harness: oracles, benches, gates
docs/                            # architecture, performance, and porting docs
reference/                       # pinned upstream Lua 5.4.7 C source (the oracle)
```

## Development

```bash
cargo build -q --bin lua-rs                       # build
TEST_TIMEOUT_S=90 ./harness/run_official_all.sh   # full upstream suite (44/44)
./harness/run_one_test.sh reference/lua-c/testes/strings.lua   # one test
python3 harness/bench/history.py                  # rebuild the perf dashboard
.claude/hooks/unsafe-budget.sh                    # unsafe-budget gate
```

## Acknowledgements & license

`lua-rs` is a port of [Lua](https://www.lua.org/) by Roberto Ierusalimschy, Luiz
Henrique de Figueiredo, and Waldemar Celes (PUC-Rio); the pinned upstream source
in `reference/` is the conformance oracle. Lua and this port are both MIT — see
[LICENSE](LICENSE).
