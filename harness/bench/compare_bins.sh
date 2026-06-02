#!/usr/bin/env bash
# compare_bins.sh — direct A/B timing for two Lua interpreter binaries.
#
# This is for packet validation when the question is "did this Rust change move
# the workload?" rather than "how far are we from reference C?". It runs the
# same harness workloads through both binaries, asserts byte-identical output,
# and reports best-of-N wall time plus max RSS. It intentionally does not append
# ledger rows; use compare.sh for dashboard/history evidence.
#
# Usage:
#   bash harness/bench/compare_bins.sh --a /tmp/lua-rs-base --b target/release/lua-rs \
#     --label-a base --label-b candidate --runs 20 --workloads gc_pressure,binarytrees
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$ROOT"

WORKLOAD_DIR="$ROOT/harness/bench/workloads"
OUT_DIR="$ROOT/harness/bench/results"
mkdir -p "$OUT_DIR"

A_BIN=""
B_BIN=""
LABEL_A="a"
LABEL_B="b"
RUNS=10
WORKLOAD_FILTER=""

while [ $# -gt 0 ]; do
    case "$1" in
        --a)         A_BIN="$2";           shift 2 ;;
        --b)         B_BIN="$2";           shift 2 ;;
        --label-a)   LABEL_A="$2";         shift 2 ;;
        --label-b)   LABEL_B="$2";         shift 2 ;;
        --runs)      RUNS="$2";            shift 2 ;;
        --workloads) WORKLOAD_FILTER="$2"; shift 2 ;;
        -h|--help)
            sed -n '2,/^set -euo/p' "${BASH_SOURCE[0]}" | sed 's/^# //; s/^#//'
            exit 0 ;;
        *) echo "unknown flag: $1" >&2; exit 1 ;;
    esac
done

[ -n "$A_BIN" ] || { echo "[err] missing --a binary" >&2; exit 2; }
[ -n "$B_BIN" ] || { echo "[err] missing --b binary" >&2; exit 2; }
[ -x "$A_BIN" ] || { echo "[err] --a binary not executable: $A_BIN" >&2; exit 2; }
[ -x "$B_BIN" ] || { echo "[err] --b binary not executable: $B_BIN" >&2; exit 2; }
case "$RUNS" in
    ''|*[!0-9]*) echo "[err] --runs must be a positive integer" >&2; exit 2 ;;
    0)           echo "[err] --runs must be >= 1" >&2; exit 2 ;;
esac

TS=$(date -u +%Y%m%dT%H%M%SZ)
COMMIT=$(git rev-parse --short HEAD 2>/dev/null || echo "unknown")
TSV="$OUT_DIR/${TS}-${COMMIT}-bin-ab.tsv"
JSON="$OUT_DIR/${TS}-${COMMIT}-bin-ab.json"

OS_NAME="$(uname -sr)"
ARCH="$(uname -m)"
CPU="$(sysctl -n machdep.cpu.brand_string 2>/dev/null || grep -m1 'model name' /proc/cpuinfo 2>/dev/null | cut -d: -f2- | sed 's/^ *//' || echo 'unknown')"

run_out() {
    local bin="$1"
    local workload="$2"
    "$bin" "$workload" 2>&1
}

measure_one() {
    local bin="$1"
    local workload="$2"
    local tmp real rss parsed rss_kb
    tmp=$(mktemp)
    case "$(uname -s)" in
        Darwin)
            /usr/bin/time -lp "$bin" "$workload" >/dev/null 2>"$tmp"
            real=$(awk '$1=="real" {print $2; exit}' "$tmp")
            rss=$(awk '/maximum resident set size/ {print $1; exit}' "$tmp")
            ;;
        *)
            /usr/bin/time -f '%e %M' "$bin" "$workload" >/dev/null 2>"$tmp"
            parsed=$(awk '/^[0-9.]+ [0-9]+$/ {r=$1; k=$2} END {if (r != "") print r, k}' "$tmp")
            real=$(printf '%s' "$parsed" | awk '{print $1}')
            rss_kb=$(printf '%s' "$parsed" | awk '{print $2}')
            [ -n "$rss_kb" ] && rss=$((rss_kb * 1024))
            ;;
    esac
    rm -f "$tmp"
    if [ -z "${real:-}" ] || [ -z "${rss:-}" ]; then
        echo "[err] failed to parse /usr/bin/time output for $bin $workload" >&2
        return 1
    fi
    printf "%s %s\n" "$real" "$rss"
}

best_of_n() {
    local bin="$1"
    local workload="$2"
    local best_real="" best_rss="" pair real rss
    for _ in $(seq 1 "$RUNS"); do
        pair=$(measure_one "$bin" "$workload") || return 1
        real=$(echo "$pair" | awk '{print $1}')
        rss=$(echo "$pair" | awk '{print $2}')
        if [ -z "$best_real" ] || awk -v a="$real" -v b="$best_real" 'BEGIN{exit !(a < b)}'; then
            best_real="$real"
        fi
        if [ -z "$best_rss" ] || awk -v a="$rss" -v b="$best_rss" 'BEGIN{exit !(a > b)}'; then
            best_rss="$rss"
        fi
    done
    printf "%s %s\n" "$best_real" "$best_rss"
}

{
    printf '# lua-rs binary A/B compare\n'
    printf '# timestamp_utc: %s\n' "$TS"
    printf '# commit:        %s\n' "$COMMIT"
    printf '# os:            %s\n' "$OS_NAME"
    printf '# arch:          %s\n' "$ARCH"
    printf '# cpu:           %s\n' "$CPU"
    printf '# runs:          %d (reporting best wall-clock, max RSS)\n' "$RUNS"
    printf '# %s: %s\n' "$LABEL_A" "$A_BIN"
    printf '# %s: %s\n' "$LABEL_B" "$B_BIN"
    printf '#\n'
    printf 'workload\t%s_wall_s\t%s_wall_s\t%s_over_%s_wall_ratio\t%s_rss_kb\t%s_rss_kb\t%s_over_%s_rss_ratio\tmatch\n' \
        "$LABEL_A" "$LABEL_B" "$LABEL_B" "$LABEL_A" "$LABEL_A" "$LABEL_B" "$LABEL_B" "$LABEL_A"
} > "$TSV"

JSON_ROWS=""
TOTAL_A=0
TOTAL_B=0

for wpath in "$WORKLOAD_DIR"/*.lua; do
    wname=$(basename "$wpath" .lua)
    if [ -n "$WORKLOAD_FILTER" ]; then
        echo ",$WORKLOAD_FILTER," | grep -q ",$wname," || continue
    fi

    echo "==> $wname" >&2
    out_a=$(run_out "$A_BIN" "$wpath")
    out_b=$(run_out "$B_BIN" "$wpath")
    match="ok"
    [ "$out_a" = "$out_b" ] || match="diff"

    pair_a=$(best_of_n "$A_BIN" "$wpath")
    pair_b=$(best_of_n "$B_BIN" "$wpath")
    a_wall=$(echo "$pair_a" | awk '{print $1}')
    a_rss=$(echo "$pair_a" | awk '{print $2}')
    b_wall=$(echo "$pair_b" | awk '{print $1}')
    b_rss=$(echo "$pair_b" | awk '{print $2}')

    wall_ratio=$(awk -v a="$b_wall" -v b="$a_wall" 'BEGIN{if (b>0) printf "%.3f", a/b; else print "NaN"}')
    rss_ratio=$(awk -v a="$b_rss" -v b="$a_rss" 'BEGIN{if (b>0) printf "%.3f", a/b; else print "NaN"}')
    a_rss_kb=$(awk -v b="$a_rss" 'BEGIN{printf "%.0f", b/1024}')
    b_rss_kb=$(awk -v b="$b_rss" 'BEGIN{printf "%.0f", b/1024}')

    printf '%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\n' \
        "$wname" "$a_wall" "$b_wall" "$wall_ratio" "$a_rss_kb" "$b_rss_kb" "$rss_ratio" "$match" >> "$TSV"

    if [ -n "$JSON_ROWS" ]; then JSON_ROWS="$JSON_ROWS,"; fi
    JSON_ROWS="$JSON_ROWS{\"workload\":\"$wname\",\"${LABEL_A}_wall_s\":$a_wall,\"${LABEL_B}_wall_s\":$b_wall,\"${LABEL_B}_over_${LABEL_A}_wall_ratio\":$wall_ratio,\"${LABEL_A}_rss_kb\":$a_rss_kb,\"${LABEL_B}_rss_kb\":$b_rss_kb,\"${LABEL_B}_over_${LABEL_A}_rss_ratio\":$rss_ratio,\"match\":\"$match\"}"

    TOTAL_A=$(awk -v t="$TOTAL_A" -v a="$a_wall" 'BEGIN{printf "%.4f", t+a}')
    TOTAL_B=$(awk -v t="$TOTAL_B" -v a="$b_wall" 'BEGIN{printf "%.4f", t+a}')
done

OVERALL_RATIO=$(awk -v a="$TOTAL_B" -v b="$TOTAL_A" 'BEGIN{if (b>0) printf "%.3f", a/b; else print "NaN"}')

{
    printf '{\n'
    printf '  "timestamp_utc": "%s",\n' "$TS"
    printf '  "commit": "%s",\n' "$COMMIT"
    printf '  "os": "%s", "arch": "%s", "cpu": "%s",\n' "$OS_NAME" "$ARCH" "$CPU"
    printf '  "runs_per_workload": %d,\n' "$RUNS"
    printf '  "labels": {"a": "%s", "b": "%s"},\n' "$LABEL_A" "$LABEL_B"
    printf '  "binaries": {"%s": "%s", "%s": "%s"},\n' "$LABEL_A" "$A_BIN" "$LABEL_B" "$B_BIN"
    printf '  "totals": {"%s_wall_s": %s, "%s_wall_s": %s, "%s_over_%s_wall_ratio": %s},\n' \
        "$LABEL_A" "$TOTAL_A" "$LABEL_B" "$TOTAL_B" "$LABEL_B" "$LABEL_A" "$OVERALL_RATIO"
    printf '  "rows": [%s]\n' "$JSON_ROWS"
    printf '}\n'
} > "$JSON"

echo >&2
echo "==> results:" >&2
echo "    tsv:  $TSV" >&2
echo "    json: $JSON" >&2
echo >&2
cat "$TSV"
