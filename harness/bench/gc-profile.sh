#!/usr/bin/env bash
# Write end-of-run GC counters for one lua-rs workload.
#
# This is telemetry, not a ledgered benchmark. It runs the normal release
# binary and writes a TSV with collection counts, heap cohorts, latest
# mark/sweep counters, and intern-table size.
#
# Usage:
#   bash harness/bench/gc-profile.sh gc_pressure
#   PROFILE_REPEAT=10 bash harness/bench/gc-profile.sh binarytrees
#   PROFILE_LUA_EVAL='for i=1,100 do dofile("harness/bench/workloads/binarytrees.lua") end' \
#     bash harness/bench/gc-profile.sh binarytrees_x100

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$ROOT"

WORKLOAD="${1:?usage: $0 <workload-name>}"
RS_BIN="$ROOT/target/release/lua-rs"
WORKLOAD_FILE="$ROOT/harness/bench/workloads/${WORKLOAD}.lua"
PROFILE_LUA_EVAL="${PROFILE_LUA_EVAL:-}"
PROFILE_REPEAT="${PROFILE_REPEAT:-1}"

case "$PROFILE_REPEAT" in
    ''|*[!0-9]*) echo "[err] PROFILE_REPEAT must be a positive integer" >&2; exit 2 ;;
    0)           echo "[err] PROFILE_REPEAT must be >= 1" >&2; exit 2 ;;
esac

[ -x "$RS_BIN" ] || { echo "[err] release binary missing: $RS_BIN" >&2; exit 2; }
if [ -z "$PROFILE_LUA_EVAL" ]; then
    [ -f "$WORKLOAD_FILE" ] || { echo "[err] workload not found: $WORKLOAD_FILE" >&2; exit 2; }
fi

TS=$(date -u +%Y%m%dT%H%M%SZ)
COMMIT=$(git rev-parse --short HEAD 2>/dev/null || echo "unknown")
WORKLOAD_LABEL="$WORKLOAD"
if [ -z "$PROFILE_LUA_EVAL" ] && [ "$PROFILE_REPEAT" -gt 1 ]; then
    escaped_workload=${WORKLOAD_FILE//\\/\\\\}
    escaped_workload=${escaped_workload//\"/\\\"}
    PROFILE_LUA_EVAL="for _profile_i = 1, ${PROFILE_REPEAT} do dofile(\"${escaped_workload}\") end"
    WORKLOAD_LABEL="${WORKLOAD}_x${PROFILE_REPEAT}"
fi

OUT_DIR="$ROOT/harness/bench/profiles/gc-profile/${TS}-${COMMIT}-${WORKLOAD_LABEL}"
mkdir -p "$OUT_DIR"

export LUA_RS_GC_PROFILE="$OUT_DIR/gc.tsv"

if [ -n "$PROFILE_LUA_EVAL" ]; then
    echo "==> running $RS_BIN -e <PROFILE_LUA_EVAL> ($WORKLOAD_LABEL)" >&2
    "$RS_BIN" -e "$PROFILE_LUA_EVAL" >"$OUT_DIR/stdout.txt" 2>"$OUT_DIR/stderr.txt"
else
    echo "==> running $RS_BIN $WORKLOAD_FILE" >&2
    "$RS_BIN" "$WORKLOAD_FILE" >"$OUT_DIR/stdout.txt" 2>"$OUT_DIR/stderr.txt"
fi

echo "==> GC report: $LUA_RS_GC_PROFILE" >&2
column -t -s $'\t' "$LUA_RS_GC_PROFILE"
