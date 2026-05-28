# lua-cli

The command-line interpreter for [lua-rs](https://github.com/ianm199/lua-rs), a
Lua 5.4 implementation written in safe Rust. The crate is `lua-cli`; the binary
it installs is `lua-rs`.

```bash
cargo install lua-cli
lua-rs -e 'print(("safe rust"):upper())'   # SAFE RUST
```

## Usage

```bash
lua-rs                          # REPL
lua-rs script.lua               # run a file
lua-rs -e 'print(1 + 2)'        # run a one-liner
echo 'print("hi")' | lua-rs -   # read from stdin
lua-rs -v                       # version
```

It mirrors the standard `lua` CLI: a bare argument is a script filename, `-e`
runs a chunk, `-` reads from stdin, and no arguments on a terminal starts the
REPL.

## What it is

A from-scratch reimplementation of PUC-Rio Lua 5.4.7 (lexer, parser, bytecode
compiler, register VM, garbage collector, coroutines, standard library) with no
C dependency. It runs the unmodified upstream Lua 5.4.7 test suite against the
binary and passes 44/44, and it runs the stock LuaRocks 3.11.1 client for
pure-Lua rocks. It is competitive with reference C (about 1.3x geomean wall
time), not faster, and is not LuaJIT.

To embed Lua in a Rust program instead of running it from the shell, use
[`lua-rs-runtime`](https://crates.io/crates/lua-rs-runtime).

Full documentation, benchmarks, and source:
[github.com/ianm199/lua-rs](https://github.com/ianm199/lua-rs).

## License

A port of [Lua](https://www.lua.org/) (Roberto Ierusalimschy, Luiz Henrique de
Figueiredo, and Waldemar Celes, PUC-Rio). Lua and this port are both
MIT-licensed.
