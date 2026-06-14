# Idiomatization Sprint 1 — live spec + recipe ledger

Owner: Fable (supervisor: design sign-off + verification). Execution: Opus
subagents per subsystem. Plan of record: `docs/IDIOMATIZATION_ROADMAP.md`
(Stage-2; read first). This is the live checklist, the Phase-0 scaffolding
(recipe format + graduation template + gate template), and the recipe ledger.

Baseline (2026-06-13): bytecode-parity oracle GREEN across the bench corpus at
main 5abb986; `omnilua` 0.2.0; official suite passing.

## Checklist (tick only with evidence)

- [x] P0: scaffolding — recipe format, graduation declaration template, gate
      template (this doc). Exercised end-to-end by the P1a pilot.
- [x] P1a: LEXER (lua-lex) idiomatized on `idiom/lexer` — bytecode parity (bench
      all-OK + broad-corpus counts identical) + behavioral suite green (oracle
      165, official 33/33, literals/errors, check.sh 5.1/5.4); 5 recipe entries +
      graduation doc (`crates/lua-lex/GRADUATED.md`) + 17 unit tests. (Not yet
      merged — branch awaiting review; do NOT push/tag per task scope.)
- [ ] P1b: PARSER (lua-parse) idiomatized + merged (if P1a clean)
- [ ] P1c: CODEGEN (lua-code) idiomatized + merged (if P1a/P1b clean)
- [ ] REFLECT: `docs/IDIOMATIZATION_REFLECTION_1.md` written with Phase-2
      go/no-go (REQUIRED before any Phase-2 work)
- [ ] CLOSE: all PRs merged CI-green; board row closed

## Phase-0 scaffolding

### The gate (Phase 1 — bytecode-parity-net subsystems)
A subsystem is idiomatized only when ALL are green:
1. **Bytecode parity**: `python3 harness/bench/bytecode-parity.py <targets>`
   byte-identical to `luac -l -l` (allowlist `bytecode-parity-allow.txt`
   unchanged — do NOT add entries to dodge a regression). Run against BOTH the
   bench corpus AND a broad set of official-test `.lua` files (lexer/parser
   need wide token/grammar coverage; pass the file list as argv).
2. **Behavioral suite**: `harness/run_official_all.sh` (full pass) +
   `cargo test -p omnilua --test multiversion_oracle` (165) + the
   lexical-error/line-number behavioral tests (errors.lua, the syntax-error
   and line-attribution cases, `specs/oracle/check.sh 5.1`..`5.5`).
3. Crate gates: `cargo test -p <crate>`, `cargo test --workspace`,
   `cargo check --target wasm32-unknown-unknown`.
These subsystems are COLD (run at load, not per-op) — no perf arbiter needed;
bytecode parity is the structural oracle and it SURVIVES idiomatizing the
producer (you change the internals; the emitted bytecode must not move).

### Recipe-catalogue format
Each idiomatization records, in this doc's "Recipe ledger" section, entries of:
- **Pattern name** (e.g. `c-charptr-scan -> peekable-iterator`)
- **Before** (the C-port shape, 1-3 lines) → **After** (the idiomatic shape)
- **Behavioral invariant that replaced the structural one**: what you now
  trust instead of "matches llex.c line N" (e.g. "token stream yields identical
  bytecode; lexical errors byte-identical per errors.lua").
- **Caveats / where it doesn't apply.**

### Graduation declaration (per subsystem)
On merge, each idiomatized subsystem gets a short `## Graduated: <crate>` note
(in its crate CLAUDE.md or a `GRADUATED.md`) stating: the C correspondence is
intentionally gone; the oracle that now guards it; what a future debugger should
trust instead of the C source. This is the load-bearing artifact — it tells the
next person the structural crutch is removed and what replaced it.

## Recipe ledger
(append transformation recipes here as subsystems graduate)

### P1a — lua-lex (the pilot), 2026-06-14

Five transformations, one commit each, the Sprint-1 gate green after every one
(bytecode parity bench all-OK + broad-corpus divergent-op counts identical to
baseline; multiversion_oracle 165; official 33/33; literals.lua + errors.lua;
check.sh 5.1 57/0, 5.4 7/0; `unsafe` stayed 0).

A note that shaped every recipe below: **the public boundary is `lua-parse`.**
`lua-parse` calls `lua_lex::{next,lookahead,set_input,lex_error,syntax_error,
sem_error,token2str}`, constructs `LexState`/`Token`/`TokenValue`/`ZIO`/
`LexBuffer` by their public fields, and reads the `TK_*` `i32` codes. So every
idiomatization had to stay **inside** those signatures. The recurring lesson:
*idiomatize the internals up to the boundary, then stop and document the
boundary as deliberate* — half the value of a recipe is knowing where it must
not reach.

---

**Recipe: `c-index-triple → single-cursor reader`**
- Pattern: a C buffer-pointer struct carrying `(n /* remaining */, p /* pos */,
  buf)` where `n` is hand-decremented in lockstep with `p`.
- Before:
  ```rust
  if self.n > 0 { self.n -= 1; let b = self.current_chunk[self.p]; self.p += 1; b as i32 }
  else { self.fill() } // fill sets n = chunk.len()-1, p = 0, returns chunk[0]
  ```
- After: keep only the cursor; derive "remaining" from `chunk.len() - cursor`.
  ```rust
  match self.chunk.get(self.cursor) { Some(&b) => { self.cursor += 1; b as i32 }
                                      None => self.fill() }  // fill: cursor = 1
  ```
- Invariant that replaced the structural one: the unit test `zio_yields_bytes_then_eoz`
  (byte sequence then a *stable* `EOZ`, empty source immediately `EOZ`) plus the
  whole bytecode-parity net — the cursor feeds every token.
- Caveat: preserve the *empty-chunk-without-committing* behaviour (an empty chunk
  reports `EOZ` but does not advance, so an interactive reader can be re-polled).
  Collapsing `None` and empty-chunk into one arm is fine **only** because both
  already produced `EOZ` without committing.

**Recipe: `c-int-status + out-param → Option`**
- Pattern: a C function returning `0`/nonzero status and writing its real result
  through an out-pointer (`int f(in, T *out)`).
- Before:
  ```rust
  let mut obj = LuaValue::Nil;
  if str2num(bytes, &mut obj) == 0 { return Err(lex_error(..., "malformed number", TK_FLT)); }
  match obj { Int(i) => ..., Float(f) => ..., _ => unreachable!() }
  ```
- After: a private wrapper that reduces the convention to `Option`, then `match`:
  ```rust
  fn parse_numeral(bytes: &[u8]) -> Option<LuaValue> { /* 0 => None else Some(out) */ }
  match parse_numeral(...) { None => Err(...), Some(Int(i)) => ..., Some(Float(f)) => ... }
  ```
- Invariant that replaced the structural one: `numeral_int_float_boundary` +
  `float_only_versions_widen_integer_literals` unit tests, and the malformed-number
  error path (line + `TK_FLT` tag) checked by `errors.lua`.
- Caveat: the *callee's* signature was out of scope (`lua_vm::object::str2num`
  lives in another crate); idiomatize at the **callsite** with a wrapper, don't
  chase the convention across the crate boundary.

**Recipe: `borrow-dance copy → named extraction method`**
- Pattern: `let v = buf[..].to_vec(); use(&mut owner, &v)` blocks whose `.to_vec()`
  exists *only* to end an immutable borrow before a `&mut` call — papered over
  per-site with a `PORT NOTE`.
- Before (×3, plus two inline trailing-NUL trims):
  ```rust
  // PORT NOTE: copy to drop the borrow before new_string
  let buf = ls.buff.as_slice();
  let content: Vec<u8> = buf[sep..buf.len()-sep].to_vec();
  ```
- After: name the *intent* on the buffer type; document the owned copy **once**
  on the method as a real ownership requirement, not a C idiom.
  ```rust
  let content = ls.buff.trim_ends(sep);   // and to_owned_text(), without_trailing_nul()
  ```
- Invariant that replaced the structural one: `lexbuffer_extraction_helpers`,
  `long_brackets_with_levels`, `hex_escape_decodes_to_bytes`, and the `near '...'`
  snippet exactness in `errors.lua` (which depends on the NUL-trim).
- Caveat: the copy is **genuinely required** (the callee mutates `ls`); the recipe
  is about *naming* it, not eliminating it. Don't try to hand back a borrow — that
  fights the borrow checker for no behavioral gain.

**Recipe: `integer-tag dispatch → internal enum at a stable boundary`**
- Pattern: a hot `match` on a raw integer that also crosses a public boundary as
  an integer; you want the typed-match ergonomics without changing the boundary.
- Before:
  ```rust
  match ls.current {                       // i32, EOZ buried as a guard
      c if c == b'=' as i32 => ...,
      c if c == EOZ => return Ok(TK_EOS),
      c => { if is_lalpha(c) ... }
  }
  ```
- After: an **internal** enum for the lexer's own classification; the emitted
  token kind still leaves as `i32`.
  ```rust
  enum Peek { Byte(u8), Eoz }
  fn peek(ls) -> Peek { match u8::try_from(ls.current) { Ok(b) => Byte(b), Err(_) => Eoz } }
  match peek(ls) { Peek::Byte(b'=') => ..., Peek::Eoz => return Ok(TK_EOS),
                   Peek::Byte(b'0'..=b'9') => ..., Peek::Byte(c) => ... }
  ```
- Invariant that replaced the structural one: the whole bytecode-parity net
  (token kinds unchanged) + `peek_maps_byte_and_eoz`, `multichar_symbols_and_dots`,
  `version_gated_operators`.
- Caveat: this is the **only** way the "tag → enum" idiom applies when the tag is a
  cross-crate boundary. Resolve the deferred "TokenKind enum" note *internally*;
  do not push the enum out to `Token.kind` (that would force `lua-parse` changes).
  `u8::try_from` maps EOZ (`-1`) cleanly to the `Err` arm — no sentinel handling.

**Recipe: `crutch removal on graduation`**
- Pattern: once a subsystem is idiomatic, its line-by-line C scaffolding is pure
  noise that actively misleads (it claims a correspondence that no longer holds).
- Action: delete the `# C source` verbatim-C doc blocks, the `file.c:NNN` refs,
  the `*.tsv` annotation comments, the C-correspondence `PORT NOTE`s, and the
  Phase-A/B stub `TODO`s (≈480 lines here). Add a `GRADUATED.md` + a module-doc /
  PORT-STATUS-trailer declaration of the new oracle.
- **Keep-list (the load-bearing half):** behavioral comments survive — every
  version gate, deliberate harness deviations (the `#`-shebang hack), correctness
  rationale (`\r\n` pairing, the `\u{}` check-before-shift order, capacity-vs-len
  in `resize`), and the new idiomatic API docs. Distinguish *"this is what the C
  did"* (delete) from *"this is why the behavior is what it is"* (keep).
- Caveat: comment-only, but it can still break the build (a blank line can detach
  a `///` from its item; an orphaned ```` ``` ```` fence). Gate it the same as a
  code change. Best done after the structural transformations land, not before —
  you want the idiomatic shape in place so it's obvious which comments are now
  redundant.

## Verdict ledger
(append per-subsystem outcomes — graduated OR honest-negative-with-reason)

### P1a — lua-lex: **GRADUATED** (2026-06-14)

5/5 planned transformations landed, each its own commit, the full gate green
after every one. `GRADUATED.md` written; module doc + PORT STATUS trailer
declare the new oracle. 17 crate-local unit tests added (tier-2 net). `unsafe`
0 → 0. No entry added to `bytecode-parity-allow.txt`. `lua-parse` untouched.

**Honest-negatives (transformations attempted/considered and deliberately not
forced):**
- *Pushing `TokenKind` out to `Token.kind`* — the original deferred Phase-B note
  asked for a `TokenKind` enum replacing `kind: i32`. NOT done at the boundary:
  `lua-parse` and the error formatters read the `TK_*` `i32` codes directly, so a
  boundary enum would require editing `lua-parse` (out of scope) and provides no
  behavioral gain. Resolved internally via `Peek` instead; recorded the `i32`
  boundary as deliberate in `Token`'s doc. (Not a parity break — a scope wall.)
- *Returning `Err(...)` from the error constructors* — making `lex_error` etc.
  return `Result`/`!` instead of a bare `LuaError` would read marginally more
  idiomatic, but `lua-parse` wraps them as `Err(lex_error(...))` across the crate
  boundary, so the by-value return is a public contract. Left as-is and documented
  as the boundary rather than churned.
- *Moving `ZIO`/`LexBuffer` to `lua_vm::zio`* — explicitly out of scope (a
  separate planned refactor). Idiomatized in place; one `TODO` each marks the move.

No transformation had to be reverted for a parity/behavior break — every gate
stayed green on the first landing.
