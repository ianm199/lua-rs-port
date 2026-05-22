#!/usr/bin/env bash
# Stop hook: every .rs file under crates/ must end with a PORT STATUS trailer.
# Format defined in PORTING.md §12.

set -uo pipefail

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$ROOT"

fail=0

while IFS= read -r f; do
    # Skip auto-generated and main.rs entry points without a translated body
    if [ "$(wc -c < "$f")" -lt 50 ]; then continue; fi

    if ! tail -25 "$f" | grep -q "PORT STATUS"; then
        echo "[trailer-required] FAIL: $f missing PORT STATUS trailer" >&2
        fail=1
        continue
    fi

    # Required fields
    for field in source target_crate confidence todos port_notes unsafe_blocks notes; do
        if ! tail -25 "$f" | grep -q "$field:"; then
            echo "[trailer-required] FAIL: $f trailer missing field '$field'" >&2
            fail=1
        fi
    done
done < <(find crates -name '*.rs' 2>/dev/null)

exit "$fail"
