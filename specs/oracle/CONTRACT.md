# Oracle contract: which reference binaries, built how

The differential oracle (`specs/oracle/diff_one.sh`, `check.sh`) compares our
version-selected `lua-rs` against reference C binaries. This pins exactly which
binaries and what their compat-flag configuration is, so "is X a bug?" has an
unambiguous answer.

## The contract

**Oracle = the unmodified upstream `make macosx` build of each pinned release**,
with no extra `-D` flags. That's what `make` produces from a clean tarball and
what real users run, so it's the defensible target.

- `lua-5.3.6` → `/tmp/lua-refs/bin/lua5.3.6`
- `lua-5.4.7` → `/tmp/lua-refs/bin/lua5.4.7`
- `lua-5.5.0` → `/tmp/lua-refs/bin/lua5.5.0`

Rebuild: `curl -sSL https://www.lua.org/ftp/lua-<v>.tar.gz | tar xz && (cd lua-<v> && make macosx)`.

## Empirically pinned compat behaviors (the ambiguous ones)

Probed directly on the binaries above (not inferred from luaconf.h, which is
layered through `LUA_COMPAT_5_x` umbrellas):

| Behavior | 5.3.6 | 5.4.7 | 5.5.0 |
|---|---|---|---|
| `__le` derived from `__lt` (only `__lt` defined) | **yes** | **yes** | (removed) |
| compat-math (`math.pow`/`atan2`/`cosh`/…) | **yes** | n/a here | no (`LUA_COMPAT_MATHLIB` off) |
| `global` usable as an ordinary identifier (`local global = 5`) | yes (not reserved) | yes | **yes** (`LUA_COMPAT_GLOBAL` on) |
| `global a, b` declaration statement | n/a | n/a | **yes** |

## What this decides

- **5.5 `global` is CONTEXTUAL, not a reserved keyword.** With `LUA_COMPAT_GLOBAL`
  on (the upstream default), `global` is only special at the start of a statement
  when it introduces a declaration; everywhere else it is a normal name. Our
  current implementation reserves it unconditionally on the 5.5 path, which both
  rejects valid identifier uses and panics (`F8`). The fix must make it contextual.

- **`__le`-from-`__lt` is part of the 5.3 AND 5.4 contract** (both default builds
  derive it). Our impl errors instead. Because this is a single cross-version
  behavior that also affects the 5.4 baseline, it is **pre-existing 5.4 port debt**,
  tracked separately from the 5.3/5.5 multiversion work, not fixed in this branch.

- **compat-math is part of the 5.3 contract** but is 5.3-completeness work, out of
  scope for the current "fix the 5.5 crash + central semantics" pass.
