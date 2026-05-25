# Examples

Runnable Lua programs that exercise `lua-rs`. Each runs on the reference
PUC-Rio Lua 5.4.7 interpreter and on `lua-rs` with identical output.

Run one with the installed binary:

```bash
lua-rs examples/fibonacci.lua
```

or from a source build:

```bash
cargo build --bin lua-rs
./target/debug/lua-rs examples/fibonacci.lua
```

| File | Shows |
|---|---|
| [`fibonacci.lua`](fibonacci.lua) | recursion, table memoization, integer `//` vs float `/` |
| [`coroutines.lua`](coroutines.lua) | coroutines as lazy generators and producer/consumer |
| [`oop.lua`](oop.lua) | object-orientation and inheritance via metatables |
| [`patterns.lua`](patterns.lua) | Lua string patterns: `match`, `gmatch`, `gsub` |
| [`errors.lua`](errors.lua) | `pcall`, table error objects, `<close>` to-be-closed variables |
| [`wasm-browser/`](wasm-browser/) | static browser playground for the published `lua-rs-wasm` package |

Run them all:

```bash
for f in examples/*.lua; do echo "== $f =="; lua-rs "$f"; done
```
