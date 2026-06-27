# #234 — Multi-version backend seam: implementation spec

**Status:** spec for review (deep-spec → codex-review → execute).
**Parent:** `specs/WEBLUA_MULTIVERSION_API_SPEC.md` (the design); this doc is the
*implementation* contract that reconciles that design with the code as built.
**Predecessor:** slice 1 (the host→Lua number-model seam: `LossyIntPolicy`,
`lower_host_int`, `LuaVersion::number_model`) already landed in 0.3.7.

---

## 0. The central reconciliation (read this first)

`WEBLUA_MULTIVERSION_API_SPEC.md` §4.1/§6 specifies internal dispatch as an
`enum Engine` with **per-version backend structs** (`v51::Engine`, `v53::Engine`,
…), each holding "version-specific concrete state (5.4 opcode tables; 5.1
function-environment machinery)", plus a §6.5 plan for 5.1/5.2 as a **separate
core**.

**That part of the design is superseded by what was actually built, and this spec
does not implement it.** omniLua runs all five versions (5.1–5.5) from a *single
versioned core*: the version is resolved once in a cold path
(`GlobalState.lua_version`, the `legacy_for` flag) and the hot bytecode dispatch
loop is version-free. This is deliberate and load-bearing — `CLAUDE.md` and the
multiversion playbook both pin it ("One core, version chosen at runtime … resolve
the version once in a cold path and never branch per-opcode. Version-gated compat
code is load-bearing"). The full official suites for all five versions already
pass against this single core.

Building `v51::Engine`/`v53::Engine`/… now would be a large refactor that
**fights a working architecture** to reach a goal that architecture has already
reached (multi-version from one binary, version chosen at runtime, common cases
byte-identical). It is the textbook premature abstraction. The closed-enum
performance argument in §4.1 (slim single-version builds collapsing to a no-op
dispatch) is real but is a *future build-size* lever, not part of making the
multi-version surface usable — and it is orthogonal to everything below.

**What §4.1's `Backend` trait actually wants, in this codebase, is a
version-indexed capability/divergence table — data, not VM structs.** The trait's
listed contract (`number_model()`, `lossy_int_policy()`, `open_libs` roster,
`gc_surface`, *the divergence registry*) is exactly a per-version descriptor.
`number_model()` already lives on `LuaVersion`. This spec adds the rest of that
descriptor as data hanging off `LuaVersion`, and makes the **divergence registry
the single source of truth** the spec §3.4 asks for. That is the realized
`Backend`-as-contract: *trait-as-contract becomes table-as-contract*, because our
dispatch is one core, not N.

So the implementable, strategically-valuable core of #234 is:

> **Make the version support matrix a single source of truth, queryable at the API
> boundary, and give the host a typed `Unsupported` error (plus a pre-check) when
> it asks a host-API verb for a feature the active version lacks.**

This directly closes the gap flagged in the embedding-API audit: *the
multi-version differentiator is largely inert at the API level.* After this, a
host can ask `lua.supports(Feature::Utf8Lib)`, render the matrix, and get a typed
error instead of a bare Lua "index nil" when it drives a version-absent feature.

---

## 1. Scope

### In scope
- **A. `Feature` enum + support-matrix registry** in `lua-types`, single source
  of truth, derived from the §3.4 table. `LuaVersion::supports(Feature) -> bool`,
  `LuaVersion::features()` (iterate the supported set), and the inverse for
  rendering a matrix.
- **B. Typed `Unsupported { feature, version }`** integrated into the public
  `omnilua::Error` model, with detection (`Error::unsupported()`,
  `Error::is_unsupported()`) and a constructor that also yields a faithful
  message for `Display`.
- **C. Wire `Unsupported` into the host-API divergence points that exist today**,
  proving the pattern end-to-end. Concrete target: the GC control surface (#231)
  — `gc().is_running()` on a pre-5.2 instance currently surfaces a raw Lua
  "invalid option" error; reclassify it to `Unsupported { GcIsRunning, version }`.
  Add `Lua::supports` as the host pre-check.

### Out of scope (explicitly)
- **Per-version `Engine`/`Backend` structs** and the §6.5 "separate 5.1/5.2 core"
  — superseded by the single core (§0). If a future slim-build size lever wants
  `#[cfg]`-gated cores, that is its own issue with its own justification.
- **Intercepting *script-level* feature use** (e.g. a 5.1 script calling
  `utf8.len` → typed `Unsupported`). That is a script runtime error
  ("attempt to index a nil value (global 'utf8')"), already oracle-correct, and
  reclassifying it would need deep per-callsite VM hooks for no real host benefit.
  `Unsupported` is for **host-API verbs**, where we own the entry point.
- **Typed `LossyIntConversion`** as a *secondary, optional* upgrade (§5). Slice 1
  already gives the correct *behavior* (error-on-inexact by default); promoting
  its string message to a typed divergence is polish, sequenced after A–C.

---

## 2. Part A — `Feature` + support matrix (`lua-types`)

Home: `crates/lua-types/src/version.rs`, next to `LuaVersion`/`number_model`.

```rust
/// A version-divergent capability. Each variant is a row of the support matrix
/// distilled from the per-version upstream deltas (the §3.4 registry). A
/// `Feature` is *present-or-absent* by version; behavioral divergences (same call,
/// different result) are NOT features here — they are resolved inside the core.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Feature {
    IntegerSubtype,      // integer/float subtypes, math.type — 5.3+
    EnvSandbox,          // _ENV, load(.., env) — 5.2+
    FenvSandbox,         // setfenv/getfenv — 5.1 only
    GotoLabels,          // goto / ::labels:: — 5.2+
    NativeBitwise,       // & | ~ << >> and // — 5.3+
    Bit32Lib,            // bit32 library — 5.2 only
    Utf8Lib,             // utf8 library — 5.3+
    StringPack,          // string.pack/unpack/packsize — 5.3+
    ToBeClosed,          // <close>/<const>, __close, coroutine.close, warn — 5.4+
    GenerationalGc,      // collectgarbage("generational"/"incremental") — 5.4+
    GcParam,             // collectgarbage("param", ...) — 5.5
    GlobalKeyword,       // `global` decl + declared-globals scope — 5.5
    NamedVarargTable,    // function f(a, ...t), table.create — 5.5
    TableLenGcMeta,      // __len/__gc on tables (not just userdata) — 5.2+
    LtEmulatesLe,        // <= derived from __lt — 5.1–5.3 (removed 5.4+)
}
```

Single source of truth — one function, no scattered version checks:

```rust
impl LuaVersion {
    pub fn supports(self, f: Feature) -> bool {
        use Feature::*;
        use LuaVersion::*;
        match f {
            IntegerSubtype | NativeBitwise | Utf8Lib | StringPack
                                  => matches!(self, V53 | V54 | V55),
            EnvSandbox            => matches!(self, V52 | V53 | V54 | V55),
            FenvSandbox           => self == V51,
            GotoLabels | TableLenGcMeta
                                  => matches!(self, V52 | V53 | V54 | V55),
            Bit32Lib              => self == V52,
            ToBeClosed | GenerationalGc
                                  => matches!(self, V54 | V55),
            GcParam | GlobalKeyword | NamedVarargTable
                                  => self == V55,
            LtEmulatesLe          => matches!(self, V51 | V52 | V53),
        }
    }

    pub fn features(self) -> impl Iterator<Item = Feature> {
        Feature::ALL.iter().copied().filter(move |f| self.supports(*f))
    }
}
```

`Feature::ALL` is a `const [Feature; N]` next to the enum (the iteration source;
keeping it adjacent makes "add a variant → add to ALL" a one-line local edit, and
a unit test asserts `ALL.len()` equals the variant count via a match).

**Cross-check against reality (the discipline that makes this trustworthy):** a
test asserts each `supports()` row agrees with the *oracle-backed* runtime —
e.g. `V51.supports(Utf8Lib) == false` and on a real 5.1 instance
`load("return type(utf8)").eval::<String>()? == "nil"`; `V53.supports(Bit32Lib)
== false` and `type(bit32) == nil` on 5.3. The registry is not allowed to drift
from the engine it describes.

## 3. Part B — typed `Unsupported` in the public error model

`Feature` + the `Unsupported` payload live in `lua-types` (pure data over
`Feature` + `LuaVersion`); the public `omnilua::Error` carries it as a typed
side-channel (we do **not** add a variant to the internal VM `LuaError` enum,
which is matched across the whole VM — that is an invasive, layering-wrong change
for a host-API concept).

```rust
// lua-types
pub struct Unsupported { pub feature: Feature, pub version: LuaVersion }

// omnilua::Error gains:
//   divergence: Option<Unsupported>
impl Error {
    pub fn unsupported(feature: Feature, version: LuaVersion) -> Self { /* sets a
        Runtime inner with a faithful message + the typed side-channel */ }
    pub fn as_unsupported(&self) -> Option<&Unsupported>;
    pub fn is_unsupported(&self) -> bool;
}
```

The message is single-sourced: `"{feature} is not available in Lua {version}"`
(`Feature` gets a `name()/Display`). `Display`/`message_lossy` therefore keep
working for hosts that don't match the typed form, and a matching host gets the
structured `feature`/`version`.

## 4. Part C — wire it at a real host-API divergence point

Prove the pattern end-to-end on the GC surface (#231):
- `Lua::supports(Feature) -> bool` (delegates to `self.version().supports(f)`) —
  the host pre-check.
- `GcControl::is_running()` on a version where `!supports(... )`: instead of
  driving `collectgarbage("isrunning")` and surfacing the raw "invalid option"
  Lua error, short-circuit to `Err(Error::unsupported(GcIsRunning?, version))`.
  *Open question for review:* `isrunning` is absent only on 5.1 — is it worth a
  `Feature` row, or should the GC surface gate on a narrower internal predicate?
  (Leaning: a `Feature::GcIsRunning` row keeps the registry the one source of
  truth and is matchable; alternative is `version >= V52` inline, which
  re-introduces a scattered check. Recommend the row.)

This is one concrete wiring; the same shape applies to any future host verb that
names a divergent feature (a GC-mode setter, `set_environment`'s mechanism, etc.).

## 5. Part D (secondary, optional) — typed `LossyIntConversion`

Slice 1's `lower_host_int` returns a string `Runtime` error on inexact ingest
under the default policy. Optionally promote it to a typed
`Error::lossy_int_conversion(value, version)` side-channel mirroring Part B, so a
host can match the cause. Behavior is already correct; this is matchability
polish. Sequence after A–C; drop if review says the string is enough for now.

## 6. Test plan (oracle is the truth-teller)
- `lua-types`: `supports()` matrix unit tests; `Feature::ALL` completeness test.
- `omnilua` new `tests/version_support.rs`: `Lua::supports` per version;
  registry-vs-runtime cross-checks (utf8/bit32/string.pack/math.type presence on
  a real instance matches `supports()`); `is_running()` on 5.1 → `is_unsupported()`
  true with `feature`/`version` set, and on 5.4 → `Ok`.
- Gate: `cargo test --workspace`, `harness/run_official_all.sh`,
  `specs/oracle/check.sh ×5`, hooks. No behavior change to any version's suite
  (this is additive API + one error-classification change on a pre-5.2 host call).

## 7. Risks / things for the reviewer to attack
1. **Is the §0 reconciliation right** — is there a real reason to build per-version
   `Engine` structs now that I'm missing? (My claim: no; the single core already
   delivers the spec's goals and the suites prove it.)
2. **`Unsupported` as an `omnilua::Error` side-channel vs a real enum variant** —
   is the side-channel matchable enough, or do hosts need a true `enum` they can
   `match` exhaustively? (Trade-off: invasiveness of touching the VM `LuaError`.)
3. **Feature granularity** — are the bundled rows (`ToBeClosed` covering
   `<close>`+`__close`+`coroutine.close`+`warn`) too coarse for a host that wants
   to gate on just one? Split now or later?
4. **Registry drift** — the cross-check test is the guard; is it strong enough, or
   should the matrix be generated from the per-version delta docs instead of
   hand-written?
5. **`GcIsRunning` feature row vs inline `version >= V52`** — which keeps the
   "single source of truth" honest without over-rowing the enum?
