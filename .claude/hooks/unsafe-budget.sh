#!/usr/bin/env bash
# Stop hook: count `unsafe` blocks per crate vs ceiling in
# harness/unsafe-budgets.toml. Fail if any crate exceeds its ceiling.
#
# Reads `harness/unsafe-budgets.toml`; default ceiling for unlisted crates: 0.

set -uo pipefail

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
BUDGETS="$ROOT/harness/unsafe-budgets.toml"

if [ ! -f "$BUDGETS" ]; then
    echo "[unsafe-budget] missing $BUDGETS" >&2
    exit 2
fi

fail=0
for crate_dir in "$ROOT"/crates/*/; do
    crate=$(basename "$crate_dir")
    # extract ceiling from TOML (simple, no full parser)
    ceiling=$(awk -F'[= \t#]+' -v c="\"$crate\"" '$1==c {print $2; exit}' "$BUDGETS")
    ceiling=${ceiling:-0}
    # sanity: must be a non-negative integer
    case "$ceiling" in
        ''|*[!0-9]*) ceiling=0 ;;
    esac

    # Count actual `unsafe` keyword USAGE — not the word in comments / trailers.
    # Match `unsafe` followed by one of {fn,impl,trait,extern,block,{} (the
    # only syntactic contexts where it's a real keyword). Exclude lines that
    # are inside a single-line comment (start with optional whitespace + //).
    count=0
    if [ -d "$crate_dir/src" ]; then
        count=$(grep -rhnE '\bunsafe[[:space:]]+(fn|impl|trait|extern|block|\{)' \
                    "$crate_dir/src" --include='*.rs' 2>/dev/null \
                | grep -vE '^\s*//' \
                | wc -l | tr -d ' ')
    fi

    if [ "$count" -gt "$ceiling" ]; then
        echo "[unsafe-budget] FAIL: crate '$crate' has $count unsafe occurrences, ceiling is $ceiling" >&2
        fail=1
    fi
done

exit "$fail"
