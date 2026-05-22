#!/usr/bin/env bash
# Phase A oracle: byte-diff `luac -o` output against our Rust compiler.
# Usage: ./diff-bytecode.sh <program.lua>
#
# Exits 0 on byte-identical output, 1 otherwise. Writes the diff to
# results/<program>.bytecode.diff on failure.

set -euo pipefail

if [ "$#" -ne 1 ]; then
    echo "usage: $0 <program.lua>" >&2
    exit 2
fi

PROG="$1"
NAME="$(basename "$PROG" .lua)"
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
RESULTS="$ROOT/harness/oracle/results"
LUAC="$ROOT/reference/lua-5.4.7/src/luac"
RUST_LUAC="$ROOT/target/release/lua-rs-luac"

mkdir -p "$RESULTS"

# Reference bytecode
"$LUAC" -o "$RESULTS/$NAME.ref.luac" "$PROG"

# Our bytecode
if [ ! -x "$RUST_LUAC" ]; then
    echo "[diff-bytecode] $RUST_LUAC not built — Phase A not complete" >&2
    echo "FAIL: lua-rs-luac not built" > "$RESULTS/$NAME.bytecode.diff"
    exit 1
fi
"$RUST_LUAC" -o "$RESULTS/$NAME.ours.luac" "$PROG"

if cmp -s "$RESULTS/$NAME.ref.luac" "$RESULTS/$NAME.ours.luac"; then
    echo "PASS $NAME"
    exit 0
else
    diff <(xxd "$RESULTS/$NAME.ref.luac") <(xxd "$RESULTS/$NAME.ours.luac") \
        > "$RESULTS/$NAME.bytecode.diff" || true
    echo "FAIL $NAME (see $RESULTS/$NAME.bytecode.diff)"
    exit 1
fi
