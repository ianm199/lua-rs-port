#!/usr/bin/env bash
# check_ghost_abstractions.sh — bidirectional ghost abstraction guard.
#
# Behavior:
#   1. Parse docs/GHOST_ABSTRACTION_REGISTER.md (every ```yaml block).
#   2. For each entry with status: active or retiring:
#      - Grep each pattern across crates/ and harness/*.sh.
#      - If zero hits -> WARN ("entry can be promoted to retired").
#      - If hits outside the declared file(s) -> FAIL ("ghost has drifted").
#   3. Grep codebase for known new-ghost patterns not covered by any entry -> FAIL.
#   4. Exit 0 if all checks pass; exit 1 on any FAIL condition.
#
# Modes:
#   --dry-run : never fail; print findings only. (Default behavior in standalone use.)
#   --strict  : fail on any FAIL condition. (Used by hook integration.)

set -uo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"

MODE="dry-run"
case "${1:-}" in
  --strict) MODE="strict" ;;
  --dry-run|"") MODE="dry-run" ;;
  *) printf 'usage: %s [--strict | --dry-run]\n' "$0" >&2; exit 2 ;;
esac

REGISTER="$ROOT/docs/GHOST_ABSTRACTION_REGISTER.md"
if [ ! -f "$REGISTER" ]; then
  printf '[err] register missing: %s\n' "$REGISTER" >&2
  exit 2
fi

CRATES_DIR="$ROOT/crates"
HARNESS_DIR="$ROOT/harness"

ok_count=0
warn_count=0
fail_count=0

emit_ok()   { printf '[OK]   %s %s\n' "$1" "$2"; ok_count=$((ok_count + 1)); }
emit_warn() { printf '[WARN] %s %s\n' "$1" "$2"; warn_count=$((warn_count + 1)); }
emit_fail() { printf '[FAIL] %s %s\n' "$1" "$2"; fail_count=$((fail_count + 1)); }

SELF_BASENAME="$(basename "$0")"

grep_codebase() {
  local pattern="$1"
  local results=""
  [ -d "$CRATES_DIR" ] && results+="$(grep -rEn "$pattern" "$CRATES_DIR" 2>/dev/null | sed "s|^$ROOT/||" || true)"$'\n'
  if [ -d "$HARNESS_DIR" ]; then
    while IFS= read -r shfile; do
      [ "$(basename "$shfile")" = "$SELF_BASENAME" ] && continue
      results+="$(grep -EHn "$pattern" "$shfile" 2>/dev/null | sed "s|^$shfile:|${shfile#$ROOT/}:|" || true)"$'\n'
    done < <(find "$HARNESS_DIR" -maxdepth 1 -name '*.sh')
  fi
  printf '%s' "$results" | grep -v '^[[:space:]]*$' || true
}

# ── Parse register and run checks ────────────────────────────────────────────
# The while-read loop pipes from awk and runs in a subshell, so we
# accumulate findings in a temp file to share across the shell.

TMPFILE="$(mktemp /tmp/ghost_check.XXXXXX)"
trap 'rm -f "$TMPFILE"' EXIT

FINDINGS="$TMPFILE"

check_entry() {
  local name="$1"
  local status="$2"
  local files_raw="$3"
  local patterns_raw="$4"

  if [ "$status" = "retired" ]; then
    printf 'OK\t%s\tstatus=retired (skipped)\n' "$name" >> "$FINDINGS"
    return
  fi

  if [ "$status" != "active" ] && [ "$status" != "retiring" ]; then
    printf 'WARN\t%s\tunrecognised status="%s"\n' "$name" "$status" >> "$FINDINGS"
    return
  fi

  while IFS= read -r pattern; do
    [ -z "$pattern" ] && continue

    hit_lines="$(grep_codebase "$pattern")"

    if [ -z "$hit_lines" ]; then
      printf 'WARN\t%s\tpattern not found (candidate for retired): %s\n' "$name" "$pattern" >> "$FINDINGS"
      continue
    fi

    drift=""
    while IFS= read -r line; do
      [ -z "$line" ] && continue
      hit_path="${line%%:*}"
      rel_path="${hit_path#$ROOT/}"
      declared=0
      while IFS= read -r declared_file; do
        [ -z "$declared_file" ] && continue
        declared_base="${declared_file%%:*}"
        if [ "$rel_path" = "$declared_base" ]; then
          declared=1
          break
        fi
      done <<< "$files_raw"
      if [ "$declared" -eq 0 ]; then
        drift="$drift $rel_path"
      fi
    done <<< "$hit_lines"

    if [ -n "$drift" ]; then
      printf 'FAIL\t%s\tpattern drifted outside declared files [%s] found in:%s\n' "$name" "$pattern" "$drift" >> "$FINDINGS"
    else
      printf 'OK\t%s\tpattern in-bounds: %s\n' "$name" "$pattern" >> "$FINDINGS"
    fi
  done <<< "$patterns_raw"
}

# Extract each yaml block and parse fields
current_name=""
current_status=""
current_files=""
current_patterns=""
in_files=0
in_patterns=0
in_block=0

while IFS= read -r line; do
  if [ "$line" = '```yaml' ]; then
    in_block=1
    current_name=""
    current_status=""
    current_files=""
    current_patterns=""
    in_files=0
    in_patterns=0
    continue
  fi

  if [ "$line" = '```' ] && [ "$in_block" -eq 1 ]; then
    in_block=0
    if [ -n "$current_name" ]; then
      check_entry "$current_name" "$current_status" "$current_files" "$current_patterns"
    fi
    continue
  fi

  [ "$in_block" -eq 0 ] && continue

  if printf '%s' "$line" | grep -qE '^name:'; then
    current_name="$(printf '%s' "$line" | sed 's/^name:[[:space:]]*//')"
    in_files=0; in_patterns=0
    continue
  fi

  if printf '%s' "$line" | grep -qE '^status:'; then
    current_status="$(printf '%s' "$line" | sed 's/^status:[[:space:]]*//')"
    in_files=0; in_patterns=0
    continue
  fi

  if printf '%s' "$line" | grep -qE '^files:'; then
    in_files=1; in_patterns=0; continue
  fi

  if printf '%s' "$line" | grep -qE '^patterns:'; then
    in_patterns=1; in_files=0; continue
  fi

  if printf '%s' "$line" | grep -qE '^[a-z_A-Z]'; then
    in_files=0; in_patterns=0
  fi

  if [ "$in_files" -eq 1 ]; then
    item="$(printf '%s' "$line" | sed 's/^[[:space:]]*-[[:space:]]*//')"
    if [ -n "$item" ]; then
      current_files="${current_files}${item}"$'\n'
    fi
  fi

  if [ "$in_patterns" -eq 1 ]; then
    item="$(printf '%s' "$line" | sed 's/^[[:space:]]*-[[:space:]]*//')"
    if [ -n "$item" ]; then
      item="${item#\"}"
      item="${item%\"}"
      item="${item#\'}"
      item="${item%\'}"
      current_patterns="${current_patterns}${item}"$'\n'
    fi
  fi

done < "$REGISTER"

# ── Emit findings from temp file ─────────────────────────────────────────────
while IFS=$'\t' read -r level name reason; do
  case "$level" in
    OK)   emit_ok   "$name" "$reason" ;;
    WARN) emit_warn "$name" "$reason" ;;
    FAIL) emit_fail "$name" "$reason" ;;
  esac
done < "$FINDINGS"

# ── New-ghost heuristics ─────────────────────────────────────────────────────

check_new_ghost_pattern() {
  local label="$1"
  local pattern="$2"
  shift 2
  local allowlist=("$@")

  hits="$(grep_codebase "$pattern")"
  [ -z "$hits" ] && return

  unregistered=""
  while IFS= read -r hit; do
    [ -z "$hit" ] && continue
    skip=0
    for allowed in "${allowlist[@]}"; do
      if printf '%s' "$hit" | grep -qF "$allowed"; then
        skip=1; break
      fi
    done
    [ "$skip" -eq 0 ] && unregistered="${unregistered}"$'\n'"  ${hit}"
  done <<< "$hits"

  if [ -n "$unregistered" ]; then
    emit_fail "new-ghost:${label}" "unregistered ghost pattern '${pattern}' found:${unregistered}"
  fi
}

check_new_ghost_pattern \
  "phase-b-no-op" \
  "phase-b no-op" \
  "state.rs"

check_new_ghost_pattern \
  "phase-b-reconcile-outside-allowlist" \
  "todo!.*phase-b-reconcile" \
  "state_stub.rs" "api.rs" "reconcile_types.sh"

check_new_ghost_pattern \
  "todo-phase-b-in-code" \
  "todo!.*phase-b" \
  "state.rs" "state_stub.rs" "object.rs" "auxlib.rs" "tagmethods.rs" "api.rs" \
  "reconcile_types.sh" "dispatch_compiler_fixer.sh" "implement_loop.sh"

check_new_ghost_pattern \
  "always-returns-some-in-docs" \
  "always returns Some" \
  "gc.rs" "state.rs"

# Detect new placeholder() functions outside the known list
check_new_ghost_pattern \
  "new-placeholder-fn" \
  "pub fn placeholder" \
  "value.rs" "proto.rs" "string.rs" "userdata.rs" "closure.rs"

# Detect duplicate pub struct definitions not already in the register
# Registered duplicates that are known ghosts with their own entries:
REGISTERED_DUPS="LuaTable Instruction LexBuffer LexState ZIO LuaDebug"

dup_names=$(
  [ -d "$CRATES_DIR" ] && grep -rEh "^pub struct [A-Z][A-Za-z]+" "$CRATES_DIR" 2>/dev/null \
    | sed 's/^pub struct \([A-Za-z_]*\).*/\1/' \
    | sort | uniq -d || true
)
if [ -n "$dup_names" ]; then
  while IFS= read -r name; do
    [ -z "$name" ] && continue
    already_registered=0
    for rdup in $REGISTERED_DUPS; do
      [ "$name" = "$rdup" ] && already_registered=1 && break
    done
    [ "$already_registered" -eq 1 ] && continue
    locations=$(grep -rEl "^pub struct $name" "$CRATES_DIR" 2>/dev/null | tr '\n' ' ')
    emit_fail "new-ghost:duplicate-pub-struct-${name}" \
      "struct '${name}' defined in multiple files: ${locations}"
  done <<< "$dup_names"
fi

# ── Summary ──────────────────────────────────────────────────────────────────
printf '\nsummary: %d OK, %d WARN, %d FAIL\n' "$ok_count" "$warn_count" "$fail_count"

if [ "$fail_count" -gt 0 ] && [ "$MODE" = "strict" ]; then
  exit 1
fi
exit 0
