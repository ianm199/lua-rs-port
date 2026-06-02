#!/usr/bin/env bash
# Write end-of-run GC counters for one lua-rs workload.
#
# This is telemetry, not a ledgered benchmark. It runs the normal release
# binary and writes a TSV with collection counts, heap cohorts, latest
# mark/sweep counters, and intern-table size.
#
# Usage:
#   bash harness/bench/gc-profile.sh gc_pressure
#   PROFILE_LUA_EVAL='for i=1,100 do dofile("harness/bench/workloads/binarytrees.lua") end' \
#     bash harness/bench/gc-profile.sh binarytrees_x100

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$ROOT"

WORKLOAD="${1:?usage: $0 <workload-name>}"
RS_BIN="$ROOT/target/release/lua-rs"
WORKLOAD_FILE="$ROOT/harness/bench/workloads/${WORKLOAD}.lua"
PROFILE_LUA_EVAL="${PROFILE_LUA_EVAL:-}"

[ -x "$RS_BIN" ] || { echo "[err] release binary missing: $RS_BIN" >&2; exit 2; }
if [ -z "$PROFILE_LUA_EVAL" ]; then
    [ -f "$WORKLOAD_FILE" ] || { echo "[err] workload not found: $WORKLOAD_FILE" >&2; exit 2; }
fi

TS=$(date -u +%Y%m%dT%H%M%SZ)
COMMIT=$(git rev-parse --short HEAD 2>/dev/null || echo "unknown")
OUT_DIR="$ROOT/harness/bench/profiles/gc-profile/${TS}-${COMMIT}-${WORKLOAD}"
mkdir -p "$OUT_DIR"

export LUA_RS_GC_PROFILE="$OUT_DIR/gc.tsv"

if [ -n "$PROFILE_LUA_EVAL" ]; then
    echo "==> running $RS_BIN -e <PROFILE_LUA_EVAL>" >&2
    "$RS_BIN" -e "$PROFILE_LUA_EVAL" >"$OUT_DIR/stdout.txt" 2>"$OUT_DIR/stderr.txt"
else
    echo "==> running $RS_BIN $WORKLOAD_FILE" >&2
    "$RS_BIN" "$WORKLOAD_FILE" >"$OUT_DIR/stdout.txt" 2>"$OUT_DIR/stderr.txt"
fi

echo "==> GC report: $LUA_RS_GC_PROFILE" >&2
column -t -s $'\t' "$LUA_RS_GC_PROFILE"
