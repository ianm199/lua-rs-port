#!/usr/bin/env bash
# check-ghost-abstractions.sh — PreToolUse wrapper for the ghost abstraction guard.
#
# STAGED — NOT YET ENABLED.
#
# To enable this hook, add the following stanza to .claude/settings.json
# under "hooks" > "PreToolUse":
#
#   {
#     "matcher": "Edit|Write|MultiEdit",
#     "hooks": [
#       {
#         "type": "command",
#         "command": "$CLAUDE_PROJECT_DIR/.claude/hooks/check-ghost-abstractions.sh"
#       }
#     ]
#   }
#
# Do NOT enable until `./harness/check_ghost_abstractions.sh --dry-run`
# reports zero FAIL. Once enabled, any Edit/Write/MultiEdit that introduces
# an unregistered ghost pattern or causes a registered ghost to drift will
# block the tool call and print a summary of findings.
#
# Behavior:
#   - Only fires for Edit, Write, and MultiEdit tool names.
#   - Calls harness/check_ghost_abstractions.sh in --strict mode.
#   - On FAIL (exit 1): exits 2 to block the tool call, printing findings.
#   - On WARN/OK (exit 0): exits 0, allowing the tool call to proceed.

set -uo pipefail

TOOL="${CLAUDE_TOOL_NAME:-}"
case "$TOOL" in
  Edit|Write|MultiEdit) ;;
  *) exit 0 ;;
esac

HOOK_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_DIR="$(cd "$HOOK_DIR/../.." && pwd)"
SCRIPT="$PROJECT_DIR/harness/check_ghost_abstractions.sh"

if [ ! -f "$SCRIPT" ]; then
  printf '[check-ghost-abstractions] harness script not found: %s\n' "$SCRIPT" >&2
  exit 0
fi

output="$("$SCRIPT" --strict 2>&1)"
exit_code=$?

if [ "$exit_code" -ne 0 ]; then
  printf '%s\n' "$output"
  printf '\n[check-ghost-abstractions] BLOCKED: ghost abstraction check failed.\n'
  printf 'Fix the FAIL lines above, or update docs/GHOST_ABSTRACTION_REGISTER.md.\n'
  exit 2
fi

exit 0
