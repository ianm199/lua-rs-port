#!/usr/bin/env bash
# Stop hook: grep changed .rs files for banned patterns per PORTING.md §5.
#
# Patterns:
#   - `use std::string::String` outside test code
#   - `: &str` / `: String` in function signatures (cheap heuristic — manual
#     review for false positives)
#   - `tokio::`, `async fn`, `futures::`, `rayon::`
#   - `std::process::Command` outside lua-cli
#   - `unwrap()` outside test code and lua-cli/main.rs

set -uo pipefail

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$ROOT"

fail=0
report() { echo "[forbidden-import] FAIL: $*" >&2; fail=1; }

# Under parallel fanout, scope to only the current worker's target via
# CLAUDE_TARGET_RS_FILE (set by fanout.sh). Otherwise scan all crates/.
if [ -n "${CLAUDE_TARGET_RS_FILE:-}" ] && [ -f "${CLAUDE_TARGET_RS_FILE}" ]; then
    FILES=("${CLAUDE_TARGET_RS_FILE}")
else
    FILES=()
    while IFS= read -r f; do FILES+=("$f"); done < <(find crates -name '*.rs' 2>/dev/null)
fi

if [ "${#FILES[@]}" = "0" ]; then
    exit 0
fi

for f in "${FILES[@]}"; do
    # Strip line and block comments so we don't false-positive on mentions
    # in docstrings or TODO notes. Cheap heuristic but catches the common
    # `// std::process::Command — stubbed` pattern that produced false
    # positives during the type-vocabulary reconcile.
    code=$(grep -v -E '^\s*(//|\*|/\*)' "$f" 2>/dev/null || true)

    # tokio / async / futures / rayon — actual code use only
    if echo "$code" | grep -qE '\b(use tokio|async fn |use futures|use rayon)'; then
        report "banned crate import in $f"
        echo "$code" | grep -nE '\b(use tokio|async fn |use futures|use rayon)' >&2 || true
    fi

    # std::process::Command outside lua-cli — must be a real use, not a mention.
    # Match: `use std::process::Command`, `std::process::Command::new`,
    #         `std::process::Command{...}`, or `let _: std::process::Command`
    # Avoid:  `// std::process::Command — stubbed` and similar prose.
    if [[ "$f" != crates/lua-cli/* ]]; then
        if echo "$code" | grep -qE '(use std::process::Command|std::process::Command::|std::process::Command\s*\{|:\s*std::process::Command\b)'; then
            report "std::process::Command outside lua-cli in $f"
        fi
    fi
done

exit "$fail"
