export const meta = {
  name: 'phaseD-5.2-bridge',
  description: 'Phase D part 1: Lua 5.2 (the bridge) — float-only number model on the modern _ENV core, 5.2 stdlib roster, syntax gates. Lift the V52 construction refusal only if it passes the oracle battery vs lua5.2.4 with no regression to 5.3/5.4/5.5.',
  phases: [
    { title: 'Design', detail: 'read-only: catalogue 5.2 deltas vs the modern core by probing lua5.2.4; decide the float-only mechanism (gate vs fork); extend oracle scripts; edit plan' },
    { title: 'Implement', detail: 'sequential oracle-gated: float-only numbers, 5.2 syntax gates, 5.2 stdlib roster, lift V52 refusal' },
    { title: 'Verify', detail: 'read-only: 5.2 battery vs lua5.2.4 + smoke scripts; confirm 5.3/5.4/5.5 unregressed; honest parity verdict' },
  ],
}

const ROOT = '/Users/ianmclaughlin/PycharmProjects/rustExperiments/lua-rs-port/.claude/worktrees/git-issues'

const CTX = [
  'Repo: ianm199/lua-rs (pure-Rust Lua), branch finish-5.2-bridge (off main). Goal: add Lua 5.2 support — the BRIDGE to the 5.1 legacy family. Strategy doc: ' + ROOT + '/specs/LUA_5_1_PORT_SPEC.md ("5.2 first: float-only + _ENV, no fenv"). Upstream delta map: ' + ROOT + '/specs/research/5.1-5.2-upstream.md.',
  '',
  'WHY 5.2 IS TRACTABLE (the key insight): 5.2 ALREADY uses the modern _ENV globals model (it REMOVED getfenv/setfenv and OP_GETGLOBAL). So 5.2 = the existing modern _ENV core MINUS the dual int/float number model (5.2 is FLOAT-ONLY: LUA_NUMBER=double, no integer subtype, no math.type), MINUS integer division `//`, MINUS bitwise operators, MINUS int-specific stdlib, PLUS the 5.2 roster (bit32, getfenv/setfenv ABSENT, _ENV present, goto present, table.pack/unpack, NO utf8). This is bounded — it reuses all the _ENV machinery. fenv is a 5.1-only problem and is OUT OF SCOPE here.',
  '',
  'FLOAT-ONLY MECHANISM (per the spike, the chosen pragmatic path — NOT a LuaValue fork): keep the dual LuaValue enum but enforce float-only BEHAVIOR via version gates at the production sites, exactly as the merged spike commit d8ec33f did partially. Reference it: `git show d8ec33f -- crates/lua-vm/src/object.rs crates/lua-vm/src/vm.rs crates/lua-vm/src/state.rs` (it suppresses the `.0` on integer-valued floats and gates out math.type/tointeger/maxinteger/mininteger under V51/V52, verified vs lua5.1.5). The seams to make float-only on V51/V52: (a) the lexer number scanner must never emit Int — all numeric literals are Float; (b) arithmetic must never PRODUCE an Int (no integer add/sub/mul; `/` already float; there is no `//` operator in 5.2); (c) tostring uses %.14g with `.0` suppression (spike did this); (d) the math roster has no int functions; (e) string->number coercion always yields float. The oracle (lua5.2.4) is the judge of each.',
  '',
  'THE ENGINE (follow it; do not re-invent):',
  '- Oracle = the unmodified make-macosx reference binary /tmp/lua-refs/bin/lua5.2.4 (also lua5.1.5 present; lua5.3.6/5.4.7/5.5.0 for cross-version no-regression). NOTE: the official 5.2 conformance suite is NOT bundled, so the oracle here is DIFFERENTIAL PROBING + a hand-built battery, not an official-suite sweep. Build a thorough 5.2 battery; probe adversarially from the 5.2 manual (https://www.lua.org/manual/5.2/), not from our source. Example scripts at /tmp/lua-refs/lua-5.1.5/test/*.lua can smoke-test (many are 5.1/5.2-compatible).',
  '- Differential oracle: ' + ROOT + '/specs/oracle/diff_one.sh currently supports only 5.3/5.4/5.5 (case stmt ~line 10). EXTEND it (and check.sh) to accept 5.1 and 5.2 (ref=/tmp/lua-refs/bin/lua5.1.5 / lua5.2.4). Add a 5.2 battery section to check.sh.',
  '- The version seam: LuaVersion (crates/lua-types/src/version.rs) already has V51/V52 with number_model()==FloatOnly; is_supported() currently EXCLUDES them. Lifting V52 = add it to is_supported() + the runtime new_versioned guard (crates/lua-rs-runtime/src/lib.rs) + the CLI LUA_RS_VERSION parse (crates/lua-cli). Do NOT lift V51 here (that needs fenv — separate phase).',
  '- CI tests: extend ' + ROOT + '/crates/lua-rs-runtime/tests/multiversion_oracle.rs with v52_* tests (Lua::new_versioned(LuaVersion::V52) + the load+pcall wrapper). Capture every expected value from lua5.2.4.',
  '- Gate (must stay green): cargo build --workspace ; cargo test --workspace --features lua-rs-runtime/derive ; ' + ROOT + '/specs/oracle/check.sh {5.4,5.3,5.5} (no regression) AND the new check.sh 5.2 battery. The modern versions MUST NOT regress (all 5.2 behavior is V52-gated).',
  '- HONESTY RULE: lift the V52 refusal ONLY if the 5.2 battery passes broadly and the smoke scripts run. If a sub-area cannot reach parity cleanly, leave the refusal in place OR mark 5.2 alpha and document the exact gaps — do NOT ship a 5.2 that masquerades as working. Report faithfully.',
].join('\n')

phase('Design')
const designAreas = [
  ['numbers', 'FLOAT-ONLY number model for 5.2. Probe lua5.2.4 exhaustively: literal forms (1, 1.0, 0x10, 1e3, .5) and their tostring; arithmetic results and tostring (3+4 -> "7" not "7.0"? confirm; 10/2; 2^2; 7%3; -5); table keying with float keys (t[1]=x; t[1.0]; #t); for-loop counters (for i=1,3); string.format("%d",x) with float; tonumber; comparisons; math.floor/ceil return type. For EACH, record lua5.2.4 output and what lua-rs V52 currently does (it is refused — temporarily probe via a one-line is_supported tweak in a scratch build, or reason from the V53 behavior + the spike). Identify every seam where Int leaks under V52. Write ' + ROOT + '/specs/followup/5.2-numbers.md.'],
  ['syntax_roster', 'SYNTAX + STDLIB ROSTER for 5.2. Probe lua5.2.4: which operators/syntax must be REJECTED (`//`, `&`,`|`,`~`,`<<`,`>>` bitwise, `<const>`/`<close>` attributes, integer-division) and which are PRESENT (`goto`/labels, `..`); the exact parse-error messages. The stdlib roster: bit32 present? math roster (atan2/cosh/... via LUA_COMPAT_MATHLIB — confirm), math.type ABSENT, getfenv/setfenv ABSENT (removed in 5.2), _ENV present, table.pack/unpack/move?, utf8 ABSENT, string.pack ABSENT, _VERSION="Lua 5.2", goto. Write ' + ROOT + '/specs/followup/5.2-syntax-roster.md with the exact present/absent list + messages + impl seams (parser gates in crates/lua-parse, roster gates in crates/lua-stdlib mirroring the existing V53/bit32 pattern).'],
]
const design = await parallel(designAreas.map(function (a) {
  const key = a[0], desc = a[1]
  return function () {
    return agent(CTX + '\n\nDESIGN (' + key + ') — READ-ONLY (run binaries, read code; do NOT edit source — a scratch is_supported tweak you REVERT is ok for probing). ' + desc + '\n\nEverything must be reproduced against lua5.2.4. Also state, for your area, whether the gate-based float-only approach is sufficient or whether any case forces a deeper change (be specific). Return a ~10-line summary + the top fixes.',
      { label: 'design:' + key, phase: 'Design', agentType: 'general-purpose' })
  }
}))

phase('Implement')
const fixOrder = [
  ['oracle-5.2-harness', 'Extend specs/oracle/diff_one.sh and specs/oracle/check.sh to accept 5.1 and 5.2 (binaries lua5.1.5/lua5.2.4). Add a thorough 5.2 battery to check.sh covering the float-only number cases and roster presence/absence from specs/followup/5.2-numbers.md and 5.2-syntax-roster.md. Commit: "test(oracle): extend harness to 5.1/5.2 + add 5.2 battery".'],
  ['float-only', 'Implement float-only number BEHAVIOR under V52 (per specs/followup/5.2-numbers.md), building on the merged spike d8ec33f pattern: lexer emits Float for all numeric literals on V51|V52; arithmetic never produces Int on V51|V52; tostring %.14g with .0 suppression; string->number always float; math.floor/ceil/etc return types per lua5.2.4. Keep V53/V54/V55 (dual model) BYTE-IDENTICAL. Gate-based, no LuaValue fork.'],
  ['syntax-roster', 'Implement the 5.2 syntax gates (reject `//`, bitwise ops, `<const>`/`<close>`; keep goto) and the 5.2 stdlib roster (bit32 present, math.type/getfenv/setfenv/utf8/string.pack ABSENT, _VERSION="Lua 5.2", compat-math present) per specs/followup/5.2-syntax-roster.md, mirroring the existing V53 roster-gate pattern. Match lua5.2.4 messages. Modern versions unchanged.'],
  ['lift-v52', 'IF the prior steps pass the battery: lift the V52 refusal — add V52 to LuaVersion::is_supported(), the runtime new_versioned/try_new_versioned guards, and the CLI LUA_RS_VERSION=5.2 parse. Add a smoke path: run a few /tmp/lua-refs/lua-5.1.5/test/*.lua scripts (the 5.2-compatible ones) under LUA_RS_VERSION=5.2 and diff vs lua5.2.4. If the battery does NOT broadly pass, DO NOT lift the refusal — instead document the gaps precisely in a report and leave V52 refused (or alpha-gated). State clearly which you did and why.'],
]
const fixes = []
for (const entry of fixOrder) {
  const key = entry[0], what = entry[1]
  const r = await agent(CTX + '\n\nIMPLEMENT (' + key + '). ' + what + '\n\nRead the relevant specs/followup/5.2-*.md first. Add CI assertions to crates/lua-rs-runtime/tests/multiversion_oracle.rs (v52_* behavior + guards that 5.3/5.4/5.5 are unchanged), then GATE: cargo build --workspace ; cargo test --workspace --features lua-rs-runtime/derive (0 failures) ; check.sh 5.4 AND 5.3 AND 5.5 (no regression) AND check.sh 5.2 (your new battery) ; reproduce fixed cases via diff_one.sh 5.2 vs lua5.2.4. If something is risky/ambiguous, STOP and document rather than guess. Commit on finish-5.2-bridge: git add -A && git commit -m "feat(5.2): ' + key + ' ...". Return what landed, gate results, anything deferred.',
    { label: 'impl:' + key, phase: 'Implement', agentType: 'general-purpose' })
  fixes.push(r)
}

phase('Verify')
const verify = await agent(CTX + '\n\nVERIFY (READ-ONLY: run binaries/tests, read code; do NOT edit). Independently judge the 5.2 bridge:\n' +
  '1. Run the full check.sh 5.2 battery and report pass/fail counts vs lua5.2.4.\n' +
  '2. Run a spread of float-only + roster + syntax cases through LUA_RS_VERSION=5.2 lua-rs vs lua5.2.4 via diff_one.sh; report MATCH/DIFF per case (numbers, tostring, table keys, for-loops, string.format %d, bit32, rejected operators, _VERSION, absent functions).\n' +
  '3. Smoke-run several /tmp/lua-refs/lua-5.1.5/test/*.lua scripts under 5.2 vs lua5.2.4.\n' +
  '4. Confirm NO regression: check.sh 5.3/5.4/5.5 + full cargo test; report counts.\n' +
  'Return a verdict table and a one-line PASS/FAIL on "Lua 5.2 is faithful enough to mark supported, with no modern-version regression". Be honest — if V52 was lifted but cases DIFF, say so and list them; if it was correctly left refused/alpha, confirm the documented gaps are real. Write ' + ROOT + '/specs/followup/PHASE_D_5.2_REPORT.md with the before/after and the verdict.',
  { label: 'verify:5.2', phase: 'Verify', agentType: 'general-purpose' })

return { design, fixes, verify }
