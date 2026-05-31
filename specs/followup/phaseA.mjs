export const meta = {
  name: 'mv-phaseA-5.4-bugs',
  description: 'Phase A: re-confirm the pre-existing 5.4 bugs (#76-79) against the reference binaries (settling contract questions), fix the clear-cut ones, add CI tests, verify no cross-version regression.',
  phases: [
    { title: 'Confirm', detail: 'parallel read-only re-verification of each bug across references + contract analysis + fix spec' },
    { title: 'Fix', detail: 'sequential oracle-gated fixes for the clear-cut bugs + CI tests' },
    { title: 'Synthesize', detail: 'report what landed, what needs a contract decision' },
  ],
}

const ROOT = '/Users/ianmclaughlin/PycharmProjects/rustExperiments/lua-rs-port/.claude/worktrees/git-issues'

const CTX = [
  'Repo: ianm199/lua-rs (pure-Rust Lua), on branch mv-followup. We just shipped 5.3/5.4/5.5 (v0.0.19).',
  'This phase addresses the PRE-EXISTING 5.4 port bugs the adversarial sweep found (filed #76-79), all verified present before the multiversion work.',
  '',
  'TOOLS (use them; never report an unreproduced finding):',
  '- Reference C binaries (the ORACLE / ground truth): /tmp/lua-refs/bin/lua5.1.5, lua5.2.4, lua5.3.6, lua5.4.7, lua5.5.0 (unmodified "make macosx" builds; the compat-flag contract is in ' + ROOT + '/specs/oracle/CONTRACT.md).',
  '- Differential oracle: ' + ROOT + '/specs/oracle/diff_one.sh <5.3|5.4|5.5> "<lua code>" prints MATCH or a DIFF block (normalizes prog-path/addresses).',
  '- Adversarial findings (repros R-A..R-G): ' + ROOT + '/specs/MULTIVERSION_ADVERSARIAL_FINDINGS.md',
  '- CI tests to extend: ' + ROOT + '/crates/lua-rs-runtime/tests/multiversion_oracle.rs (use Lua::new_versioned + the load+pcall wrapper already there).',
  '- Gate (all must STAY green; a shared-core fix must match EVERY version reference, not just one): cargo build --workspace ; cargo test --workspace --features lua-rs-runtime/derive ; ' + ROOT + '/specs/oracle/check.sh {5.4,5.3,5.5}.',
].join('\n')

phase('Confirm')
const bugs = [
  ['76', 'math.type and math.tointeger return boolean false instead of nil (a fail). Repros on each version: math.type("x"), math.tointeger(3.5), math.tointeger(2^63). Confirm the reference returns nil on 5.3/5.4/5.5; locate the impl in crates/lua-stdlib/src/math_lib.rs (math_type / math_toint).'],
  ['77', 'string.find returns a spurious trailing empty value for a pattern with magic chars but no explicit captures. Repros: select("#", string.find("hello","l+")) is 2 on the reference, 3 for us; also inspect the returned values. Confirm vs reference on 5.3/5.4/5.5; locate the impl in crates/lua-stdlib/src/string_lib.rs (find).'],
  ['78', 'The __le metamethod derived from __lt. CONTRACT-DEPENDENT: the default make-macosx builds of lua5.3.6 AND lua5.4.7 derive a<=b as not (b<a) when only __lt is defined (LUA_COMPAT_LT_LE, default on), but lua5.5.0 removed it. Confirm exactly which references derive it. Fixing this would CHANGE our 5.4 behavior (currently errors) to match the compat-built reference. Analyze whether matching the default-build reference is the right contract (cite specs/oracle/CONTRACT.md). DO NOT fix it here; classify clear-cut = NO and produce a recommendation for a human decision.'],
  ['79', 'Error-message fidelity cluster (R-D/E/F/G): (a) bad-argument errors drop the "to <fnname>" qualifier and say "got nil" vs "got no value"; (b) length, concat, and string-arith-coercion-failure errors drop the "(command line):N:" location prefix; (c) arith/unary metamethod-failure messages mislabel the SECOND operand type (e.g. negating a string reports the 2nd operand as a function); (d) uncaught errors omit the trailing "[C]: in ?" traceback frame; (e) table.concat invalid-value leaks the internal byte-array repr instead of saying table. Confirm each sub-item vs the reference on 5.4 (cross-version), classify each sub-item clear-cut vs risky, and pinpoint the impl location.'],
]
const confirms = await parallel(bugs.map(function (entry) {
  const n = entry[0], desc = entry[1]
  return function () {
    return agent(
      CTX + '\n\nCONFIRM bug #' + n + ' (READ-ONLY: do not edit source; you may run the binaries and read code). ' + desc +
      '\n\nRe-verify it is a genuine, current divergence by running BOTH our version-selected lua-rs and the matching reference across 5.3/5.4/5.5 (and 5.1/5.2 references where relevant) via diff_one.sh or direct invocation. Then write ' + ROOT + '/specs/followup/confirm-' + n + '.md with: exact repros plus our-vs-each-version-reference outputs, a CLEAR-CUT (safe to fix to match all references) vs CONTRACT-DEPENDENT/RISKY classification, the precise impl location(s) as file:line, the intended fix, and the exact CI test assertions to add. Return a ~10-line summary: confirmed? clear-cut? where? plus the headline repro.',
      { label: 'confirm:#' + n, phase: 'Confirm', agentType: 'general-purpose' }
    )
  }
}))

phase('Fix')
// Sequential on the shared mv-followup worktree (avoids conflicts; keeps
// diff_one.sh pointed at the one real binary). #78 is contract-dependent and is
// left for a human decision (see confirm-78.md).
const fixOrder = [
  ['76', 'math.type / math.tointeger must return nil, not false'],
  ['77', 'string.find must not return the spurious extra empty value'],
  ['79', 'error-message fidelity cluster: fix the CLEAR-CUT sub-items (missing "to <fnname>" qualifier, missing "(command line):N:" prefix on length/concat/string-arith, wrong 2nd-operand type, missing "[C]: in ?" tail, table.concat repr leak). Do the safe ones; if a sub-item is risky or ambiguous, leave it and note it.'],
]
const fixes = []
for (const entry of fixOrder) {
  const n = entry[0], what = entry[1]
  const r = await agent(
    CTX + '\n\nFIX bug #' + n + ': ' + what + '. First READ ' + ROOT + '/specs/followup/confirm-' + n + '.md for the confirmed spec, repros, location, and test cases. These are cross-version (shared-core) bugs: the fix must make our output match EVERY version reference (5.3/5.4/5.5) and regress none.' +
    '\n\nSteps: implement the fix; add CI assertions to crates/lua-rs-runtime/tests/multiversion_oracle.rs (cover 5.3/5.4/5.5 as appropriate); then GATE: cargo build --workspace, cargo test --workspace --features lua-rs-runtime/derive (0 failures), specs/oracle/check.sh for 5.4 and 5.3 and 5.5 (all green), and reproduce the specific repros now matching via diff_one.sh. If a part is risky/ambiguous, STOP and leave it documented rather than guess. Commit on mv-followup with: git add -A && git commit -m "fix(5.4): #' + n + ' ...". Return: what landed, gate results, anything deferred.',
    { label: 'fix:#' + n, phase: 'Fix', agentType: 'general-purpose' }
  )
  fixes.push(r)
}

phase('Synthesize')
const report = await agent(
  CTX + '\n\nSYNTHESIS. Read the confirm-*.md specs and the fix results. Write ' + ROOT + '/specs/followup/PHASE_A_REPORT.md: per bug (#76-79) confirmed?, fixed? (with gate results), or deferred-for-decision (#78 __le: present the contract analysis and a clear recommendation on whether to match the compat-built reference). Note any error-cluster sub-items left. End with the current oracle status (5.4/5.3/5.5) and what Phase B (finishing 5.3) should pick up. Return a ~15-line executive summary.',
  { label: 'synthesize', phase: 'Synthesize', agentType: 'general-purpose' }
)

return { confirms, fixes, report }
