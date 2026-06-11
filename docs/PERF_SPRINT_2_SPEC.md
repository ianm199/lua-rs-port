# Perf sprint 2 spec — started 2026-06-11

Owner: Fable (supervisor: design sign-off + verification). Execution: Opus
subagents per packet. Goal of record: `docs/PERF_SPRINT_2_GOAL.md` — read it
first; this file is the live checklist and verdict ledger, in the style of
`docs/ISSUE_BURNDOWN_SPEC.md`. Baseline evidence: stock matrix
`harness/bench/results/20260611T164856Z-b0e68f8-compare.tsv` (overall 1.47).
Board claim: `../AGENT_COORDINATION_BOARD.md` Active Work row (Fable,
2026-06-11).

Status checklist (tick only with evidence paths):

- [x] T0.1 `instr-count.sh --branch-sim` (Bc/Bcm/Bi/Bim; tool is cachegrind,
      header corrected) — PR #158; first run surfaced fibonacci Bim 3.3x vs C
      (bytecode dispatch), the CPI gap now measurable
- [x] T0.2 bash-3.2 `set -u` EXTRA_MOUNT fix; audit found no other
      empty-array bugs in harness/bench — PR #158
- [x] T0.3 `profile-hotspots.sh` agent-stall FIXED (detached watchdog
      inherited stdout/stderr and held the pipe open; fds detached, watchdog
      reaped via pkill -P) — validated under the agent harness, PR #158
- [x] T0.4 `heap-diff.sh` landed with exact-zero null test — PR #158; first
      real use produced T1's causal evidence below
- [x] T0.5 `docs/MEASUREMENT_PROTOCOL.md` written (supervisor-authored,
      2026-06-11), linked from `CLAUDE.md` §Benchmarks
- [x] T0.6 `port-harness/templates/c-to-rust/perf-packet.md` extracted
      (port-harness commit 90239a5, green proof = ISSUE_BURNDOWN_SPEC.md)
- [x] T1 UpVal mirror removal landed (commit 03c4468): UpVal 64→32 B,
      GcBox<UpVal> 104→72 B (value_layout; the goal doc's "≤64 B" bar was a
      supervisor arithmetic error — 72 B is the floor with the 40 B GcHeader,
      whose diet belongs to T3/#113). closure_ops causal chain: heap-diff
      total bytes −7.89% / peak live −10.86% with alloc count unchanged
      (100k upvals × 32 B = the measured −3,199,936 B), process max-RSS
      40.7→37.3 MB (−8.3%, interleaved ×3). Gates: oracle 165/0, canaries
      36/0, quarantine clean on coroutine/locals/closure, workspace 0 fail,
      wasm check green. UpValState deleted from the public API (0.0.x).
      #113 progress comment: pending below.
- [ ] T2 setter family: FRESH profile first; Ir down on ≥2 of
      {table_setfield_same, table_seti_same, global_settabup_same,
      table_settable_string_key}, no control regression, canaries +
      quarantine green
- [ ] T3a GC/alloc design memo (supervisor-written, dhat-quantified;
      options: size-class free lists, Vec→Box<[T]> table parts,
      sweep-time pooling, pacer cadence; SmallVec stays rejected)
- [ ] T3b memo's top-ranked bounded step implemented + full battery
- [ ] T4 safety-tax ablation measured on branch `ablation/unchecked-stack`
      (NEVER merged); matrix delta written into `docs/PERFORMANCE_MODEL.md`
- [ ] CLOSE: CHANGELOG entries, closing full compare.sh --runs 5 matrix in
      the bench ledger, board row moved to Recently Completed

## Protocol

The measurement protocol, gates, and stop conditions are normative in
`docs/PERF_SPRINT_2_GOAL.md` §"Non-negotiable measurement protocol" — they are
not restated here. Two local notes:

- Frozen baselines this sprint: name them `/tmp/lua-rs-s2-<packet>-base` and
  record the build sha next to each tick.
- Bench-host rule: one measurement process at a time across all repos;
  implementation agents may report provisional numbers under load, but every
  number that appears in a PR body or this checklist is re-measured quiet by
  the supervisor.

## Verdict ledger

(append per-packet outcomes here as they land — kept verdicts AND honest
negatives, with evidence paths)
