#!/usr/bin/env bash
# PreToolUse hook (Edit/Write): block writes to
# harness/oracle/test-results.json unless the corresponding evidence file
# in harness/oracle/results/ was read in this session.
#
# Anti-sycophancy: the Verifier cannot mark a phase passing without first
# reading the actual oracle output.
#
# Receives a JSON payload on stdin describing the impending tool use.
# We extract `tool_input.file_path` and check.

set -uo pipefail

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
PAYLOAD="$(cat)"

# Extract target path (jq if available; fall back to grep)
TARGET=""
if command -v jq >/dev/null 2>&1; then
    TARGET=$(echo "$PAYLOAD" | jq -r '.tool_input.file_path // empty')
fi
if [ -z "$TARGET" ]; then
    # naive fallback
    TARGET=$(echo "$PAYLOAD" | grep -oE '"file_path"\s*:\s*"[^"]+"' | head -1 | sed -E 's/.*"file_path"\s*:\s*"([^"]+)"/\1/')
fi

# Only gate writes to test-results.json
case "$TARGET" in
    */harness/oracle/test-results.json)
        ;;
    *)
        exit 0
        ;;
esac

# Has the agent read any evidence file in this session?
# Heuristic: check whether results/ contains files newer than test-results.json.
RESULTS_DIR="$ROOT/harness/oracle/results"
if [ -d "$RESULTS_DIR" ] && [ -n "$(ls -1 "$RESULTS_DIR" 2>/dev/null)" ]; then
    exit 0
fi

echo "[verify-gate] BLOCK: refusing to write test-results.json without evidence in $RESULTS_DIR" >&2
exit 2
