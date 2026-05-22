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

GATING_HOOKS=(
    "$ROOT/.claude/hooks/unsafe-budget.sh"
    "$ROOT/.claude/hooks/forbidden-import.sh"
    "$ROOT/.claude/hooks/type-vocabulary.sh"
    "$ROOT/.claude/hooks/trailer-required.sh"
)

failed_hooks=()
for hook in "${GATING_HOOKS[@]}"; do
    if [ ! -x "$hook" ]; then
        continue
    fi
    if ! "$hook" >/dev/null 2>&1; then
        failed_hooks+=("$(basename "$hook")")
    fi
done

if [ "${#failed_hooks[@]}" -gt 0 ]; then
    echo "[commit-on-stop] BLOCKED: ${#failed_hooks[@]} gating hook(s) failed: ${failed_hooks[*]}" >&2
    echo "[commit-on-stop] Refusing to auto-commit. Fix violations and re-run, or commit by hand." >&2
    echo "[commit-on-stop] To see failure details, run each failed hook directly:" >&2
    for h in "${failed_hooks[@]}"; do
        echo "    $ROOT/.claude/hooks/$h" >&2
    done
    exit 0
fi

git add -A 2>/dev/null
git commit -q -m "agent: auto-commit at stop ($(date -u +%Y-%m-%dT%H:%M:%SZ))" 2>/dev/null || true
exit 0
