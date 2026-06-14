# Graduated: lua-lex

Status: graduated 2026-06-14 (Idiomatization Sprint 1, Phase P1a — the pilot).
Branch of record: `idiom/lexer`. Plan: `docs/IDIOMATIZATION_ROADMAP.md`,
`docs/IDIOMATIZATION_SPRINT_1_SPEC.md`.

## What "graduated" means here

`crates/lua-lex/src/lib.rs` was originally a line-by-line port of PUC-Rio
`llex.c` + `llex.h`. As of Sprint 1 it has been idiomatized to native Rust and
**the C correspondence is intentionally gone**: the `# C source` verbatim-C
blocks, the `llex.c:NNN` line references, the `macros.tsv` / `types.tsv`
annotations, and the C-correspondence `PORT NOTE`s have all been removed. Do
**not** open `llex.c` to reason about this file — the structural mapping no
longer holds, and chasing it will mislead you. The structural oracle (fidelity
to the C source) has been deliberately retired for this crate.

## The oracle that now guards it

Behaviour is held by three nets, strongest first. A change to this crate is
verified only when all are green (this is the Sprint-1 gate):

1. **Bytecode parity (structural, strongest).** The token stream this lexer
   produces must drive `omnilua` to emit bytecode byte-identical to
   `luac -l -l`. Crucially this oracle **survives idiomatizing the producer**:
   you may rewrite the lexer internals however you like; if the emitted bytecode
   does not move, you are provably token-stream-preserving.
   - `python3 harness/bench/bytecode-parity.py` (bench corpus — all-OK).
   - `python3 harness/bench/bytecode-parity.py reference/lua-5.4.7-tests/*.lua`
     (broad token/grammar coverage). Note: the broad corpus has **pre-existing
     codegen-level divergences** (constant folding, `LOADNIL` coalescing) that
     are NOT lexer-related; the invariant is that the per-file divergent-op
     **counts do not change** — a count that moves means the token stream moved.
2. **Behavioral suite (output parity).** `cargo test -p omnilua --test
   multiversion_oracle` (165), the full official suite
   (`harness/run_official_all.sh`), and the version-gated batteries
   (`specs/oracle/check.sh 5.1`..`5.5`).
3. **Lexical-edge behavioral tests.** `literals.lua` (number/string/escape
   exactness) and `errors.lua` (lexical error wording, the `near '<token>'`
   snippets, line attribution) — the files that exercise exactly the delicate
   surface idiomatization could break.

Plus the crate's own fast net, new in Sprint 1: **`cargo test -p lua-lex`** (17
unit tests, <0.5s) — the tier-2 inner loop for this crate. It drives the lexer
end-to-end (`new_state` → `set_input` → `next`) over the gnarly cases (CRLF
pairing + line attribution, hex/utf8/decimal/`\z` escapes, long brackets,
int/float boundary, version-gated operators and reserved words).

## What a future debugger should trust instead of llex.c

- **The version gates are the behavioral invariant.** One core lexes 5.1–5.5;
  the differences (which operators exist — `<<`/`>>`/`//`/`::`; escape handling;
  reserved words `goto`/`global`; error wording like `escape sequence too large`
  vs `decimal escape too large`) are gated on `LexState.version` (a snapshot)
  and on live `state.global().lua_version`. **That distinction is load-bearing**
  (`token2str` reads the snapshot specifically). The inline comments that explain
  each gate were KEPT for exactly this reason — they describe behavior, not C.
- **The boundary is `i32`, not an enum.** `Token.kind`, `next`, `lookahead`,
  `setinput`, `lex_error`/`syntax_error`/`sem_error`/`token2str`, and the `TK_*`
  constants are the public API that `lua-parse` consumes. The lexer's *own*
  dispatch uses the internal `Peek { Byte(u8), Eoz }` enum, but the emitted token
  kind crosses the boundary as `i32`. Do not "finish" the enum out to the
  boundary — that would touch `lua-parse`.
- **Byte exactness.** Lua strings are bytes (`&[u8]`/`Vec<u8>`), never
  `String`/`&str`. The trailing-NUL trim, the delimiter strip (`trim_ends`), and
  the long-string `\n` normalization are byte operations; the oracle that proves
  them is `literals.lua`.
- **Still deferred (not part of this graduation):** `ZIO` and `LexBuffer` are
  idiomatized *in place* but still live in this crate; their planned move to
  `lua_vm::zio` is a separate refactor (one `TODO` each marks it). The
  cross-crate stubs (`intern_str`, hex/utf8 helpers) remain as wired in earlier
  phases.

## Recipes harvested

See the "Recipe ledger" in `docs/IDIOMATIZATION_SPRINT_1_SPEC.md` for the
reusable before→after patterns this pilot produced (byte-cursor, C-status →
`Option`, borrow-dance → named extraction, integer-tag dispatch → internal enum
at a stable boundary, crutch removal).
