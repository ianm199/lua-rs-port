# Multi-version playbook — how to add a Lua version to lua-rs

This is the reusable methodology for supporting another Lua version (or closing
a version's long tail) on the shared lua-rs core. It is the durable, harness-as-
product artifact distilled from bringing **5.1, 5.2, 5.3, 5.4, and 5.5** to
oracle-verified parity from one embedding API. Read it before starting version
work; refine it when it is wrong.

If you only remember one thing: **the reference binary is the only truth-teller,
and you climb the iteration ladder only as far as the current question forces.**

---

## 0. The result this codifies

lua-rs runs Lua **5.1–5.5**, selected per instance (`Lua::new_versioned`,
`LUA_RS_VERSION=5.x`), on **one** core — the bytecode dispatch loop carries no
per-version cost, so compute-bound code runs identically across versions.
Version-specific behavior lives in a handful of cold-path seams. 5.3/5.4/5.5 are
the modern dual-int/float core; 5.1/5.2 are the float-only family (5.2 keeps the
modern `_ENV` globals model, 5.1 adds fenv).

The work that got here was sequenced as oracle-gated phases, each its own
workflow and PR: finish 5.3 → traceback frame → finish 5.5 → shared-core
cross-version fidelity → 5.2 bridge → 5.1 legacy → architectural deferrals. Every
phase followed the same engine below.

---

## 1. The non-negotiables

1. **The reference binary is the oracle.** The unmodified `make macosx` builds in
   `/tmp/lua-refs/bin` (`lua5.1.5`/`lua5.2.4`/`lua5.3.6`/`lua5.4.7`/`lua5.5.0`)
   are ground truth, pinned in `specs/oracle/CONTRACT.md`. Default builds are
   binding — that means `LUA_COMPAT_*` defaults are part of the contract
   (e.g. `LUA_COMPAT_MATHLIB` is ON in 5.3/5.4, so `math.atan2` etc. are part of
   those versions; `LUA_COMPAT_MODULE` is ON in 5.1/5.2). A change that passes
   the build but no oracle has spoken on is **unverified**. Build success is not
   signal.

2. **Adversarial-first.** Derive test cases from the upstream manual, the
   official test suites, and *probing the reference binary* — **never** from our
   own Rust source. The canonical lesson: a happy-path battery hid the 5.5
   `global` block-scoping bug because every case used top-level declarations. If
   your battery was written by reading our code, it tests our code, not Lua.

3. **CI-test-per-fix.** Every fix adds an assertion whose expected value was
   captured from the reference binary:
   - `crates/lua-rs-runtime/tests/multiversion_oracle.rs` — in-process
     (`Lua::new_versioned` + the `load`+`pcall` wrapper) for value/error behavior.
   - `crates/lua-cli/tests/traceback_oracle.rs` — **spawn-the-binary** for
     anything that only appears in the CLI (tracebacks, `warn`/`__gc` stderr,
     the `[C]: in ?` frame, location prefixes). These do not surface in the
     in-process wrapper.

4. **No version ever regresses.** A shared-core change must match *every*
   affected version's reference, not just the one you are working on. The gate
   (below) runs all five.

5. **Honesty rule (anti-sycophancy).** A version is marked supported / a fix is
   claimed ONLY if the oracle says so. If a sub-area can't reach clean parity,
   leave it refused / mark it alpha / document the exact gap — do **not** ship
   something that masquerades as working. The one pre-approved documented
   exception is RNG sequence (5.1's host `rand()` is not portably bit-matchable;
   match the *contract* — ranges, arg-errors, return shape — not the sequence).

---

## 2. The tools

- `specs/oracle/diff_one.sh <ver> "<lua>"` — differential diff of one snippet vs
  the version's reference binary (normalizes prog-path + heap addresses). Accepts
  `5.1`–`5.5`.
- `specs/oracle/check.sh <ver>` — runs the version's battery and reports
  `N passed / M failed (vs reference)`. Extend the battery here when you add
  cases.
- `harness/canaries/gc/run_canaries.sh` — GC canaries (incremental + generational
  modes); run on any GC/metamethod/table change.
- The official suites that ARE bundled: `/tmp/lua-refs/lua-5.3.4-tests/*.lua` and
  `/tmp/lua-refs/lua-5.5.0-tests/*.lua` (run with the preamble
  `_soft=true; _port=true; _nomsg=true; _U=false; arg=arg or {}`, `cd` into the
  dir, absolute path to lua-rs). **5.1/5.2 conformance suites are NOT bundled** —
  for those the oracle is a hand-built battery + the 5.1.5 example programs in
  `/tmp/lua-refs/lua-5.1.5/test/*.lua` + adversarial manual probing. Say so; that
  coverage is weaker and the report must note it.

## 3. The iteration ladder — climb only as far as the question forces

| Tier | What | When |
|---|---|---|
| 1 | `cargo build -p <crate>` | does it compile? |
| 2 | `cargo test -p lua-rs-runtime --test multiversion_oracle` | the inner loop — does the behavior match the baked oracle constants? |
| 3 | `diff_one.sh <ver> "<snippet>"` | one specific divergence vs the live reference |
| 4 | `check.sh <ver>` | did I regress the version's battery? |
| 5 | one official suite file via the CLI vs reference | did I regress a real program? |
| 6 | `check.sh` ×5 + `cargo test --workspace` + GC canaries | the phase/PR gate |

Most fixes live on tiers 2–4. Tier 6 is the gate to land. Start one rung lower
than feels right; if the cheap rung is silent, that's your answer.

## 4. The gate (every PR)

```
cargo build --workspace          # must be WARNING-FREE (a stale #[expect] is a failure)
cargo test --workspace --features lua-rs-runtime/derive   # 0 failures
specs/oracle/check.sh 5.1 && ...5.2 && ...5.3 && ...5.4 && ...5.5   # all green
harness/canaries/gc/run_canaries.sh   # on GC/metamethod/table changes
```

CI also enforces `tests (rust + conformance)`, `wasm package`, and the
`unsafe budget gate`. 5.4 (the most mature) is the canary for "did the shared
core move" — it should stay byte-identical to baseline modulo RNG/timing/PID/path
noise.

---

## 5. The version seam (where per-version behavior lives)

A runtime `LuaVersion` flag, NOT a `dyn`-dispatched engine. It is mirrored to
`GlobalState.lua_version` and read **only in cold paths**, so the hot dispatch
loop is version-free.

- `crates/lua-types/src/version.rs` — `LuaVersion {V51..V55}`, `number_model()`
  (`FloatOnly` for 5.1/5.2, `Dual` for 5.3+), `is_supported()`, `version_str()`,
  `luac_version_byte()`. **Lifting a version = adding it to `is_supported()`**
  plus the `new_versioned` guard (`lua-rs-runtime/src/lib.rs`) and the CLI
  `LUA_RS_VERSION` parse (`lua-cli/src/main.rs`).
- Read `state.global().lua_version` (or `number_model()`) at the production site.
- **Where seams cluster, by subsystem:**
  - *Lexer* (`lua-lex`): number scanner (float-only literals; reject `//`/bitwise/
    `\x`/`\z`/hex-float/`p`-exponent per version), `global` contextuality.
  - *Parser* (`lua-parse`): name resolution / `_ENV` threading, `global`
    declarations, attribute syntax, for-var const-ness, goto/label scoping
    (block-scoped pre-5.4 vs function-wide 5.4+), operator availability.
  - *VM* (`lua-vm`): `number_to_str_buf` (float format), `raw_arith` (float-only
    arm), `__len`/`__pairs`/`__gc`-on-tables dispatch (inert in 5.1!), `__le`-from-
    `__lt` derivation (kept 5.1–5.4, dropped 5.5).
  - *Stdlib roster* (`lua-stdlib/src/init.rs` + per-lib bodies): a data-driven
    per-version registration table; gate individual entries (bit32, utf8,
    string.pack, math.type, compat-math, getfenv/setfenv, module) — do **not**
    fork whole modules.

---

## 6. Per-version cheat-sheet (the axes that bite)

- **5.4** — the reference core. Don't regress it.
- **5.5** — modern + dual numbers. Deltas: contextual/block-scoped/`<const>`
  `global` decls, named varargs `...t`, round-trip float `tostring`,
  `table.create`, `__le`-from-`__lt` REMOVED, `collectgarbage` param API, namewhat
  prefers `in global '<fn>'`.
- **5.3** — dual numbers, but `LUA_COMPAT_MATHLIB`/string coercion-in-bitwise/
  compat-math roster ON; many error-wording deltas; block-scoped goto.
- **5.2** — THE BRIDGE: **float-only numbers** on the **modern `_ENV` core** (no
  fenv — it was removed in 5.2). Reject `//`/bitwise/`<const>`. bit32 present,
  utf8/string.pack absent, `module` present. Do 5.2 before 5.1 to prove float-
  only in isolation from the globals model.
- **5.1** — THE LEGACY FAMILY: float-only **+ fenv globals**. Implement fenv via
  **Option B** (reuse the modern `_ENV` upvalue as the per-closure env;
  `getfenv`/`setfenv` read/write it + the thread global table — the two-slot
  distinction is the #1 fakeable bug). `__len`-on-tables is **inert** (the #1
  silent-failure trap). No goto, no bit32, C-`rand()` PRNG (sequence is a
  documented divergence). C-function environments (`LUA_ENVIRONINDEX`) are an
  acceptable documented gap.

The float-only model is **gate-based, not a `LuaValue` fork**: keep the dual enum,
enforce float-only *behavior* at the production sites (lexer emits Float, arith
never produces Int, `tostring` suppresses `.0`, `%d` truncates, math roster has no
int fns). Empirically sufficient — `Int(7)` and `Float(7.0)` are observationally
identical once `tostring` drops the `.0`.

---

## 7. The workflow shape (per phase)

Each version / long-tail batch is one phase = one Workflow = one branch = one PR:

1. **Discover / Design** (parallel, READ-ONLY): sweep the official suite and/or
   probe the reference to catalogue divergences by category; for a new version,
   decide the mechanism (gate vs fork) and extend the oracle scripts. Write the
   findings to `specs/followup/<topic>.md`.
2. **Fix** (SEQUENTIAL — fix agents share the worktree, so do not parallelize
   file-editing agents): one category per agent, each oracle-gated, CI-tested,
   committed. Fix-or-document: STOP and write a re-entry note rather than guess
   on anything architectural.
3. **Synthesize / Verify** (READ-ONLY): re-measure parity before/after, confirm
   no version regressed, write `specs/followup/<PHASE>_REPORT.md`, and emit a
   clean list of issue-titles for the architectural remainder so they get filed.

Review between phases; land each PR green before branching the next off `main`.
A separated read-only verifier (it has no write tools) is structural anti-
sycophancy — it physically cannot rubber-stamp a failing result.

---

## 8. Releasing

Per `RELEASING.md`: bump `Cargo.toml` (workspace + every dep entry),
`crates/lua-rs-runtime/README.md`, `packages/lua-rs-wasm/package.json`; `cargo
build` to refresh `Cargo.lock`; update `CHANGELOG.md`; merge the bump PR;
**tag the exact merge SHA** (`git show <sha>:Cargo.toml | grep version` first —
do NOT tag `origin/main`, it can advance). The tag push publishes irreversibly to
crates.io + npm. Lua versions are shipped alpha→beta→stable as their oracle
coverage grows; be explicit about which are which in the CHANGELOG.
