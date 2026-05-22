#!/usr/bin/env bash
# Stop hook (quality-of-life): auto-commit any uncommitted Rust/harness work
# IF the gating hooks all pass. Kills the "agent ran for two hours, results
# blown away" failure mode WITHOUT papering over real violations.
#
# Why we re-run the gating hooks here: in Claude Code's Stop chain, every
# hook runs regardless of prior exit codes. Without this gate, an agent that
# violates the type-vocabulary or unsafe-budget hooks would still get its
# work auto-committed — exactly what we saw with the unsafe-budget false
# positive that committed ldebug.c despite the chain reporting failure.
#
# Exits 0 either way (so we don't crash the harness), but only commits when
# the gating hooks all pass. If gating fails, logs loudly so the failure is
# visible in the agent transcript.

set -uo pipefail

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$ROOT"

if [ -z "$(git status --porcelain 2>/dev/null)" ]; then
    exit 0
fi

# Scope: only check the .rs files THIS agent modified. Pre-existing
# workspace debt (e.g. lua-gc unsafe-budget overage) doesn't block an
# agent who only touched lua-stdlib. We loop over modified .rs files,
# set CLAUDE_TARGET_RS_FILE per file, and run each gating hook.
modified_rs=()
while IFS= read -r f; do
    [ -z "$f" ] && continue
    [[ "$f" == crates/*.rs ]] || [[ "$f" == crates/*/*.rs ]] || [[ "$f" == crates/*/src/*.rs ]] || [[ "$f" == crates/*/src/**/*.rs ]] || continue
    [ -f "$f" ] && modified_rs+=("$f")
done < <( (git diff --name-only HEAD -- 'crates/**/*.rs'; git ls-files --others --exclude-standard -- 'crates/**/*.rs') 2>/dev/null | sort -u )

GATING_HOOKS=(
    "$ROOT/.claude/hooks/unsafe-budget.sh"
    "$ROOT/.claude/hooks/forbidden-import.sh"
    "$ROOT/.claude/hooks/type-vocabulary.sh"
    "$ROOT/.claude/hooks/trailer-required.sh"
)

failed_pairs=()
if [ "${#modified_rs[@]}" -gt 0 ]; then
    for f in "${modified_rs[@]}"; do
        for hook in "${GATING_HOOKS[@]}"; do
            [ -x "$hook" ] || continue
            if ! CLAUDE_TARGET_RS_FILE="$f" "$hook" >/dev/null 2>&1; then
                failed_pairs+=("$(basename "$hook"):$f")
            fi
        done
    done
else
    # No crate-side changes (likely a harness/docs-only edit). Skip gating.
    :
fi

if [ "${#failed_pairs[@]}" -gt 0 ]; then
    echo "[commit-on-stop] BLOCKED: ${#failed_pairs[@]} hook violation(s) on agent-modified files:" >&2
    for pair in "${failed_pairs[@]}"; do
        echo "  - $pair" >&2
    done
    echo "[commit-on-stop] Refusing to auto-commit. Fix violations and re-run, or commit by hand." >&2
    echo "[commit-on-stop] (Pre-existing debt on files THIS agent didn't modify is ignored.)" >&2
    exit 0
fi

git add -A 2>/dev/null
git commit -q -m "agent: auto-commit at stop ($(date -u +%Y-%m-%dT%H:%M:%SZ))" 2>/dev/null || true
exit 0
