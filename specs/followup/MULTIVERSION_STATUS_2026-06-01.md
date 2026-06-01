# Multi-version fidelity — refreshed status (2026-06-01, post-fix)

Re-verification of `MULTIVERSION_ADVERSARIAL_FINDINGS.md` against the **current**
code + the `/tmp/lua-refs/bin` reference binaries (5.1.5/5.2.4/5.3.6/5.4.7/5.5.0).
**The adversarial doc is now substantially stale — most of its prioritized list
is fixed.** Read this first; treat that doc as the historical hunt.

## What's now FIXED (re-verified MATCH vs reference)

The entire §6 "fix-before-trustworthy" list from the adversarial doc, except the
line-hook item, is resolved:

| Adversarial item | Probe | Status |
|---|---|---|
| F8 — `global` as identifier *panics* (5.5) | `local global = 5; print(global)` | FIXED (5) |
| F5 — 5.3 compat-math missing | `math.pow(2,10)`, `math.atan2(1,1)` | FIXED (1024.0 / 0.785…) |
| F6 — 5.3 string coercion in arith/bitwise | `"3" & 5`, `"10"+5` | FIXED (1 / 15.0) |
| F4 — 5.5 float round-trip `tostring` | `1/3`, `2^53` | FIXED (`%.17g`) |
| F1 — 5.5 `global` block-scoping | `do global Y; Y=1 end; print"after"` | FIXED |
| F2 — 5.5 `global x = expr` dropped init | `global x=7; …; print(x)` | FIXED (7) |
| F3 — 5.5 for-loop control-var read-only | `for i=1,3 do i=i+1 end` | FIXED (errors) |
| R-A/R-B/R-C/R-E/F (5.4 value+error bugs) | issues #76/#77/#78/#79 | CLOSED |
| #97 `__le`-from-`__lt` across a yield | this session | FIXED (PR #106) |

The 5.4 numeric/language/error core is byte-clean against 5.4.7 (oracle: 36/44,
the 8 DIVERGE are known PRNG/limit/locale/codegen).

## H2 "do 5.1/5.2 just masquerade as 5.4?" — REFUTED for syntax

The adversarial hunt never tested 5.1/5.2 (no binaries then). Whole-corpus sweep
now run (`harness/parity_check.sh` with `LUA_RS_VERSION` + the matching ref):
lua-rs's 5.1 mode **correctly rejects 5.1-invalid syntax with the same message as
5.1.5** — bitwise `~`/`&` (`unexpected symbol near '~'`), goto labels
(`'=' expected near 'l1'`), `<const>` attributes (`unexpected symbol near '<'`).
Not a 5.4 masquerade. The whole-file sweep's raw DIVERGE count is inflated by two
*formatting* artifacts, not behavior (see below).

## Genuine remaining gaps (verified)

1. **#92 line-hook back-edge rule (5.1–5.3).** Real: ref fires one extra
   same-line event per back-edge. Tried the "easy half" in isolation this
   session — it corrupts reported line values because the fire delta is coupled
   to bytecode line-attribution (#92 part 2). Must be fixed as one two-crate
   change, part 2 first. Full mechanism + C-source refs posted on #92.
2. **5.1 traceback format family.** 5.1 predates `luaL_traceback`; its whole
   traceback shape differs. Concretely verified: tail frame is `[C]: ?` (5.1) vs
   `[C]: in ?` (5.2+). Filed as a tracked issue. Low priority, structural.
3. **#105 — 5.1 doesn't quote special near/expected tokens** (`<eof>`,
   `<name>`…). 5.1-only; verified; needs version threaded into the lexer error
   formatter. Low priority.
4. **Long tail** (adversarial F9/F10/F11 fragments): niche 5.5 APIs
   (`utf8.offset` 2nd return, `error(nil)`→`<no error object>`), 5.3 `__ipairs`,
   and shared error-wording fragments. Re-verify individually before acting;
   several may already be fixed.

## How to re-run the sweep

```
LUA_RS_VERSION=5.3 REF=/tmp/lua-refs/bin/lua5.3.6 TMPDIR=/tmp/sw53 \
  bash harness/parity_check.sh        # inherits LUA_RS_VERSION into lua-rs
```
Caveat: whole-file cross-version diffs are noisy against the 5.4-vintage corpus
(feature mismatch + the binary-progname/chunkid-truncation in error lines, which
`norm()` doesn't scrub for `/tmp` paths). Snippet-level (`specs/oracle/diff_one.sh`)
is the higher-signal tool — that's how the items above were verified.
