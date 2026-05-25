# lua-rs

**Lua 5.4.7, reimplemented in safe Rust.**

`lua-rs` is a from-scratch Rust port of the reference [PUC-Rio Lua 5.4.7](https://www.lua.org/)
interpreter. It runs ordinary Lua programs with no C runtime dependency, and it
passes **44 / 44** of the upstream Lua test suite — the same `.lua` files the C
implementation is validated against.

[![crates.io](https://img.shields.io/crates/v/lua-cli.svg?label=crates.io%2Flua-cli)](https://crates.io/crates/lua-cli)
[![license](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
[![upstream tests](https://img.shields.io/badge/upstream%20suite-44%2F44-0f8f68.svg)](#conformance)
[![performance](https://img.shields.io/badge/perf-live%20dashboard-2f6fed.svg)](https://ianm199.github.io/lua-rs-port/harness/bench/history/)

```bash
cargo install lua-cli       # crate: lua-cli  →  binary: lua-rs
lua-rs -e 'print("hello from lua-rs")'
```

> [!NOTE]
> The package on crates.io is named **`lua-cli`**; it installs a binary named
> **`lua-rs`**. `cargo install lua-cli` is the install; `lua-rs` is what you run.

---

## Highlights

- **Passes the real Lua test suite.** Not a subset, not a lookalike — the
  upstream PUC-Rio Lua 5.4.7 `testes/` suite runs against this binary and passes
  44/44. See [Conformance](#conformance).
- **Safe Rust by default.** Most crates compile under `#![forbid(unsafe_code)]`.
  The only `unsafe` is a small, audited, budgeted core in the garbage collector
  and the optional dynamic-library loader. See [Safety model](#safety-model).
- **No C runtime.** Running a `.lua` script links no `liblua` and shells out to
  no C interpreter. It is a standalone Rust binary.
- **Competitive performance, tracked publicly.** Within ~1.3× of reference C on
  a geometric mean of wall time, faster than C on some workloads — and every
  commit's benchmark is plotted on a [live dashboard](https://ianm199.github.io/lua-rs-port/harness/bench/history/).
- **Built by an AI porting harness.** ~28k lines of C became safe Rust under a
  test-oracle-gated, multi-agent harness. That methodology is the deeper story —
  see [How it was built](#how-it-was-built).

## Installation

From crates.io (preview release `0.0.1`):

```bash
cargo install lua-cli
```

This installs the `lua-rs` binary into `~/.cargo/bin`. Confirm it is on your
`PATH`, then:

```bash
lua-rs -e 'print(("safe rust"):upper())'   # SAFE RUST
```

From source:

```bash
git clone https://github.com/ianm199/lua-rs-port
cd lua-rs-port
cargo build --release --bin lua-rs
./target/release/lua-rs -e 'print(_VERSION)'   # Lua 5.4
```

## Usage

```bash
lua-rs script.lua                 # run a Lua source file
lua-rs -e 'print(1 + 2)'          # run a one-liner
lua-rs 'print("bare source")'     # a bare non-file argument is treated as source
```

Supported today: running a script file, `-e <chunk>`, and a bare source-string
argument. There is **no REPL**, no stdin (`-`) execution, and no `--help`/`-v`
flag yet — see [Limitations](#limitations-and-non-goals).

## Conformance

The strongest claim this project makes is conformance. The repository runs the
unmodified upstream Lua 5.4.7 test files against the `lua-rs` binary through a
behavioral oracle (same input → diff stdout + exit code against reference C):

```bash
cargo build -q --bin lua-rs
TEST_TIMEOUT_S=90 ./harness/run_official_all.sh
# → 44/44 PASS
```

This is strong evidence for **Lua source/runtime compatibility**. It does *not*
imply C API or ABI compatibility (see [Limitations](#limitations-and-non-goals)).
The per-test debugging history is in
[docs/OFFICIAL_TEST_INVESTIGATIONS.md](docs/OFFICIAL_TEST_INVESTIGATIONS.md).

## Performance

Every benchmarked commit is recorded and plotted on a live, auto-built
dashboard:

### → [**ianm199.github.io/lua-rs-port** — live performance dashboard](https://ianm199.github.io/lua-rs-port/harness/bench/history/)

Each point is a `compare.sh` run: the ratio of `lua-rs` wall time to reference
PUC-Rio Lua on the same workload. **Lower is better; `1.00×` is parity with C.**

At the latest benchmarked commit, across 8 workloads:

| Metric | Value | Reading |
|---|---|---|
| Wall-time geomean | **1.27×** | ~27% slower than C on average |
| RSS geomean | **1.96×** | ~2× the memory of C |
| Best workload | **0.38×** | faster than C |
| Worst workload | **2.07×** | slowest relative workload |

The honest summary: this is **not** "faster than C." It is a memory-safe
reimplementation that is *competitive* with C — within a small constant factor
on average and ahead on some workloads — with the full per-workload trajectory
published rather than reduced to one headline number. Method and policy are in
[docs/PERFORMANCE_PRINCIPLES.md](docs/PERFORMANCE_PRINCIPLES.md).

## Safety model

`lua-rs` exposes a safe public surface over a small, audited unsafe core. Most
crates in the workspace compile under `#![forbid(unsafe_code)]` (the workspace
default). The remaining `unsafe` is explicitly budgeted, per-crate ceilings
enforced by a CI-style gate:

```bash
.claude/hooks/unsafe-budget.sh
```

The budgeted unsafe lives in:

- **`lua-gc`** — the garbage collector's heap and tracing internals.
- **`lua-cli`** — the optional `libloading`-backed dynamic-library backend (FFI
  is inherently unsafe; each block carries a `// SAFETY:` justification).

This is **not** "completely safe Rust," and the README will not claim that. It
is a safe surface over a contained, documented unsafe core. Details in
[docs/LUA_SYSTEM_DEEP_DIVE.md](docs/LUA_SYSTEM_DEEP_DIVE.md) and
[docs/PUBLISH_READINESS.md](docs/PUBLISH_READINESS.md).

## How it was built

The runtime is the artifact. The **AI-agent porting harness** is the method that
produced it — and the more reusable result.

Porting ~28k lines of C to safe Rust was driven by bounded, single-purpose
agents (translator, compiler-fixer, test-fixer, read-only verifier) gated by a
non-negotiable **oracle**: a change is unverified until the upstream test suite
or a structural diff says it matches reference C. Mechanical guardrails
(unsafe-budget ceilings, forbidden-pattern bans, required status trailers, a
verify-gate) are enforced as hooks, not vibes. The read-only verifier *cannot*
mark a test passing — anti-sycophancy by construction.

- [PORTING.md](PORTING.md) — the C→Rust translation rules agents follow.
- [HARNESS_DESIGN.md](HARNESS_DESIGN.md) — harness structure and enforcement model.
- [docs/RETROSPECTIVE_AND_PRODUCTIZATION.md](docs/RETROSPECTIVE_AND_PRODUCTIZATION.md)
  — what we learned and what a productized v2 needs.

## Roadmap

- **LuaRocks support (in progress).** The LuaRocks 3.11.1 CLI boots under
  `lua-rs` and prints its version/config output; `luarocks install <rock>` does
  not work yet (fetch/digest/install-path work remains, plus an exit-teardown
  fix). Plan: [docs/PHASE_G_LUAROCKS_PLAN.md](docs/PHASE_G_LUAROCKS_PLAN.md).
- **CLI surface.** REPL, stdin execution, and a polished `--help`/`--version`.
- **Embedding API.** A Rust-native embedding surface; a C API/ABI story is a
  longer-term, separate effort. See [docs/FUTURE_GOALS.md](docs/FUTURE_GOALS.md).

## Limitations and non-goals

- **Not LuaJIT**, and not targeting LuaJIT-level performance.
- **Not a C-ABI drop-in.** This runtime does not currently expose the Lua C
  API/ABI, so stock Lua C modules that expect `liblua` will not load unchanged.
- **Not for Lua 5.1 ecosystems** (OpenResty, Neovim's LuaJIT embedding, WoW
  addons) — this is Lua 5.4.
- No REPL, stdin execution, or polished CLI help yet.

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
cargo build -q --bin lua-rs                 # build
TEST_TIMEOUT_S=90 ./harness/run_official_all.sh   # full upstream suite (44/44)
./harness/run_one_test.sh reference/lua-c/testes/strings.lua   # one test
python3 harness/bench/history.py            # rebuild the perf dashboard
.claude/hooks/unsafe-budget.sh              # unsafe-budget gate
```

## Acknowledgements

`lua-rs` is a port of [Lua](https://www.lua.org/), created by Roberto
Ierusalimschy, Luiz Henrique de Figueiredo, and Waldemar Celes at PUC-Rio. The
upstream Lua source is pinned in `reference/` and used as the conformance
oracle. Lua is distributed under the MIT license; this port is likewise MIT.

## License

MIT — see [LICENSE](LICENSE).
