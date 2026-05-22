# ANALYSES — pre-computed cross-file lookups

The Bun project pre-computed `LIFETIMES.tsv` so per-file translation agents
didn't have to re-derive ownership decisions for every file. Same principle
applies here. These TSVs are **lookup tables**, not inference targets.

| File | Purpose | Built when | Status |
|---|---|---|---|
| `macros.tsv` | Every public macro in `lobject.h` / `lstate.h` / `llimits.h` → Rust equivalent | Before Phase A | **stub** |
| `types.tsv` | Each C struct → Rust struct, field-by-field, with chosen Rust type | Before Phase A | **stub** |
| `error_sites.tsv` | Every `luaG_runerror` / `luaD_throw` / `luaO_pushfstring`-then-throw → `Err(LuaError::...)` | Before Phase B | **stub** |
| `file_deps.txt` | Header inclusion graph + canonical crate assignment | Before Phase A | **stub** |

## Format

All TSVs: tab-separated, first row is a header row beginning with `#`. Comments start with `#` at line start.

## How to (re-)build

Currently manual + agent-assisted. Future: a dedicated build pass before Phase A starts.

## Stub content

The four files below contain only a handful of example rows to demonstrate the format. The Translator role will fail informatively on files that reference macros / types / error sites not yet in the TSVs (rather than guessing).
