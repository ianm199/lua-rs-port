#!/usr/bin/env bash
# reconcile_types.sh — parallel type-vocabulary cleanup orchestrator.
#
# Goal: drive `python3 harness/check_type_vocabulary.py --audit` from N
# enforce-mode violations to 0 by dispatching one agent per offending
# crate, in parallel.
#
# Each agent is scoped to ONE crate's files. The type-vocabulary hook
# blocks new violations. commit-on-stop only commits if the hook passes.
# That means even if an agent does something stupid, it cannot land.
#
# Usage:
#   ./harness/reconcile_types.sh            # full run
#   ./harness/reconcile_types.sh --dry-run  # show what would dispatch
#
# Output:
#   harness/reconcile/state.jsonl
#   harness/reconcile/RECONCILE_REPORT.md

set -uo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

DRY_RUN=0
[ "${1:-}" = "--dry-run" ] && DRY_RUN=1

OUT_DIR="harness/reconcile"
mkdir -p "$OUT_DIR"
STATE="$OUT_DIR/state.jsonl"
REPORT="$OUT_DIR/RECONCILE_REPORT.md"
LOG="$OUT_DIR/reconcile.log"
touch "$STATE" "$LOG"

START_TS=$(date +%s)
START_ISO=$(date -u +"%Y-%m-%dT%H:%M:%SZ")

emit() {
    local ts; ts=$(date -u +"%Y-%m-%dT%H:%M:%SZ")
    echo "[$ts] $*" | tee -a "$LOG"
}

record() {
    local action="$1" detail="${2:-}"
    local ts; ts=$(date -u +"%Y-%m-%dT%H:%M:%SZ")
    jq -c -n --arg ts "$ts" --arg action "$action" --arg detail "$detail" \
        '{ts: $ts, action: $action, detail: $detail}' >> "$STATE"
}

audit_count() {
    /usr/bin/python3 harness/check_type_vocabulary.py --audit 2>&1 | /usr/bin/awk '/^\[type-vocabulary\] FAIL/ {c++} END {print c+0}'
}

CRATES=(lua-stdlib lua-lex lua-code lua-parse lua-gc)

emit "═════════════════════════════════════════════════════════════════"
emit "Reconcile types — start $START_ISO"
emit "  Pre-audit FAIL count: $(audit_count)"
emit "  Crates targeted: ${CRATES[*]}"
emit "  Mode: $([ $DRY_RUN -eq 1 ] && echo dry-run || echo dispatch)"
emit "═════════════════════════════════════════════════════════════════"
record "run_start" ""

dispatch_one() {
    local crate="$1"
    local pidfile="$OUT_DIR/$crate.pid"
    local outfile="$OUT_DIR/$crate.out"

    local prompt="You are a Reconcile-types agent. Scope: ONE crate, '$crate'.

Repo: $ROOT. Read CLAUDE.md, PORTING.md, and crucially harness/type-vocabulary.tsv.

Your task: your crate currently defines duplicates of canonical cross-crate types
(LuaState, GlobalState, LuaError, etc.) that are owned by another crate per the
type-vocabulary registry. Replace the duplicates with canonical imports.

Step by step:

1. Run 'python3 harness/check_type_vocabulary.py --audit 2>&1 | grep $crate' to see
   exactly which types your crate is duplicating.

2. For each duplication, identify the canonical owner from harness/type-vocabulary.tsv.
   For each owner crate that your crate does not yet depend on, ADD the dep to
   crates/$crate/Cargo.toml under [dependencies]:
       <owner-crate>.workspace = true

3. Replace each local duplicate definition with:
       pub use <owner_crate_underscored>::<module>::<TypeName>;
   For lua-vm types this is usually lua_vm::state::LuaState etc.

4. After the swap, run 'cargo check -p $crate 2>&1 | head -100' to see new errors.
   These are signature/method mismatches that surface when the real type replaces
   the fake one. For each:
   - If the method on LuaState/GlobalState doesn't exist, do NOT add it (you would
     have to touch lua-vm, which is out of scope). Instead, replace the call site
     with todo!(\"phase-b-reconcile: <description>\") and a brief TODO_ARCH comment.
   - If a field access doesn't exist, same treatment.
   - If a signature mismatch is mechanical (e.g. &str vs &[u8]), fix it locally.

Hard constraints:

- Edit ONLY files under crates/$crate/ AND crates/$crate/Cargo.toml.
- Do NOT touch crates/lua-types/.
- Do NOT touch any other crate.
- Do NOT add a new 'pub struct/enum/trait/type' with a name listed in
  harness/type-vocabulary.tsv — the hook will block your commit.
- todo!(\"phase-b-reconcile: <what's missing>\") is correct for unimplemented bodies.

Reporting requirement (under 200 words):

- Number of FAIL violations in this crate before vs after.
- Cargo deps you added.
- Number of todo!() stubs you introduced.
- Top 1-2 issues you couldn't resolve and why.

You may take many turns. The type-vocabulary hook will block any commit that
introduces new vocabulary violations, so you cannot regress."

    if [ $DRY_RUN -eq 1 ]; then
        emit "  [DRY] would dispatch: $crate (~prompt length $(echo "$prompt" | wc -c) chars)"
        return 0
    fi

    emit "  dispatching: $crate"
    record "dispatch_start" "$crate"

    export CLAUDE_CONFIG_DIR="$HOME/.claude-personal"
    unset ANTHROPIC_API_KEY ANTHROPIC_AUTH_TOKEN
    export CLAUDE_CODE_MAX_OUTPUT_TOKENS="${CLAUDE_CODE_MAX_OUTPUT_TOKENS:-64000}"

    # Per-worker target hint so per-file hooks scope correctly.
    export CLAUDE_TARGET_RS_FILE="crates/$crate/src"

    (
        local out_json="$OUT_DIR/$crate.translator.json"
        local transcript="$OUT_DIR/$crate.transcript.jsonl"
        claude -p \
            --append-system-prompt "$(cat PORTING.md)" \
            --allowedTools "Read,Write,Edit,Glob,Grep,Bash(cargo check*),Bash(cargo *),Bash(rustc *),Bash(grep *),Bash(rg *),Bash(python3 harness/check_type_vocabulary.py*),Bash(cat *),Bash(head *),Bash(tail *)" \
            --permission-mode dontAsk \
            --output-format stream-json \
            --include-partial-messages \
            --verbose \
            --max-budget-usd 8.00 \
            "$prompt" \
            2>>"$OUT_DIR/$crate.stderr" \
            | tee "$transcript" >/dev/null
        jq -s 'map(select(.type == "result")) | .[-1] // {}' "$transcript" > "$out_json" 2>/dev/null || echo '{}' > "$out_json"
        cost=$(jq -r '.total_cost_usd // 0' "$out_json")
        is_err=$(jq -r '.is_error // false' "$out_json")
        emit "  done: $crate cost=\$$cost is_error=$is_err"
        record "dispatch_done" "$crate cost=$cost is_error=$is_err"
    ) >> "$outfile" 2>&1 &
    echo $! > "$pidfile"
}

for crate in "${CRATES[@]}"; do
    dispatch_one "$crate"
done

if [ $DRY_RUN -eq 1 ]; then
    emit "Dry run complete."
    exit 0
fi

emit "All 5 agents dispatched. Waiting for completion..."

# Wait for every background agent.
for crate in "${CRATES[@]}"; do
    pid=$(cat "$OUT_DIR/$crate.pid" 2>/dev/null)
    if [ -n "$pid" ]; then
        wait "$pid" 2>/dev/null || true
    fi
done

emit "All agents complete. Re-auditing..."
record "all_done" ""

POST_AUDIT=$(audit_count)
WORKSPACE_ERRORS=$(cargo check --workspace 2>&1 | /usr/bin/awk '/^error\[/ {c++} END {print c+0}')

END_TS=$(date +%s)
ELAPSED=$((END_TS - START_TS))
ELAPSED_MIN=$((ELAPSED / 60))

TOTAL_COST=0
for crate in "${CRATES[@]}"; do
    cost=$(jq -r '.total_cost_usd // 0' "$OUT_DIR/$crate.translator.json" 2>/dev/null)
    TOTAL_COST=$(awk -v a="$TOTAL_COST" -v b="$cost" 'BEGIN { printf "%.4f", a + b }')
done

{
    echo "# Type-vocabulary reconcile — report"
    echo ""
    echo "**Started**: $START_ISO"
    echo "**Ended**: $(date -u +"%Y-%m-%dT%H:%M:%SZ")"
    echo "**Elapsed**: ${ELAPSED_MIN} min"
    echo "**Total cost**: \$$TOTAL_COST"
    echo ""
    echo "## Audit before / after"
    echo ""
    echo "- enforce-mode FAIL count: ? → $POST_AUDIT (target: 0)"
    echo "- workspace cargo errors: 2+ → $WORKSPACE_ERRORS"
    echo ""
    echo "## Per-crate outcomes"
    echo ""
    echo "| Crate | Cost | is_error |"
    echo "|---|---:|---|"
    for crate in "${CRATES[@]}"; do
        cost=$(jq -r '.total_cost_usd // 0' "$OUT_DIR/$crate.translator.json" 2>/dev/null)
        is_err=$(jq -r '.is_error // false' "$OUT_DIR/$crate.translator.json" 2>/dev/null)
        echo "| $crate | \$$cost | $is_err |"
    done
    echo ""
    echo "## Final audit detail"
    echo ""
    echo '```'
    /usr/bin/python3 harness/check_type_vocabulary.py --audit 2>&1 | /usr/bin/head -60
    echo '```'
    echo ""
    echo "## Git activity"
    echo ""
    echo '```'
    git log --oneline --since="$START_ISO" | head -40
    echo '```'
} > "$REPORT"

emit "Report: $REPORT"
emit "  Post-audit FAIL count: $POST_AUDIT (was ? before)"
emit "  Workspace errors: $WORKSPACE_ERRORS"
emit "  Total cost: \$$TOTAL_COST"
emit "═════════════════════════════════════════════════════════════════"

exit 0
