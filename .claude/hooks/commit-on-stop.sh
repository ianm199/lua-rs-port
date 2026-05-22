#!/usr/bin/env bash
# Stop hook (quality-of-life): auto-commit any uncommitted Rust/harness work.
# Kills the "agent ran for two hours, results blown away" failure.
#
# Never fails — exit 0 always. Skips if no changes.

set -uo pipefail

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$ROOT"

if [ -z "$(git status --porcelain 2>/dev/null)" ]; then
    exit 0
fi

git add -A 2>/dev/null
git commit -q -m "agent: auto-commit at stop ($(date -u +%Y-%m-%dT%H:%M:%SZ))" 2>/dev/null || true
exit 0
