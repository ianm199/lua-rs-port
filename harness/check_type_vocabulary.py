#!/usr/bin/env python3
"""Check workspace-wide canonical type ownership.

This is a semantic harness check, not a Rust parser. It intentionally uses a
small regex over public-ish Rust item declarations because the failure mode we
care about is simple: an agent adds `pub struct LuaState` in the wrong crate to
make a local `cargo check -p <crate>` pass.

Default mode checks only modified files, so the hook can be adopted while known
Phase-B debt still exists. Use `--audit` to scan the full workspace and report
all vocabulary duplicates.
"""

from __future__ import annotations

import argparse
import dataclasses
import re
import subprocess
import sys
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
REGISTRY = ROOT / "harness" / "type-vocabulary.tsv"

ITEM_RE = re.compile(
    r"^\s*(?P<vis>pub(?:\([^)]*\))?)\s+"
    r"(?P<kind>struct|enum|trait|type)\s+"
    r"(?P<name>[A-Za-z_][A-Za-z0-9_]*)\b"
)


@dataclasses.dataclass(frozen=True)
class VocabularyEntry:
    name: str
    kind: str
    owner: Path
    mode: str
    notes: str


@dataclasses.dataclass(frozen=True)
class RustItem:
    name: str
    kind: str
    path: Path
    line: int


def rel(path: Path) -> str:
    try:
        return str(path.resolve().relative_to(ROOT))
    except ValueError:
        return str(path)


def load_registry(path: Path = REGISTRY) -> dict[str, VocabularyEntry]:
    entries: dict[str, VocabularyEntry] = {}
    for raw in path.read_text().splitlines():
        line = raw.strip()
        if not line or line.startswith("#"):
            continue
        parts = line.split(None, 4)
        if len(parts) < 4:
            raise SystemExit(f"{rel(path)}: malformed registry row: {raw!r}")
        name, kind, owner, mode = parts[:4]
        notes = parts[4] if len(parts) == 5 else ""
        if mode not in {"enforce", "audit"}:
            raise SystemExit(f"{rel(path)}: invalid mode for {name}: {mode}")
        entries[name] = VocabularyEntry(
            name=name,
            kind=kind,
            owner=(ROOT / owner).resolve(),
            mode=mode,
            notes=notes,
        )
    return entries


def rust_files() -> list[Path]:
    return sorted((ROOT / "crates").glob("*/src/**/*.rs"))


def changed_files() -> list[Path]:
    if target := os_environ("CLAUDE_TARGET_RS_FILE"):
        path = (ROOT / target).resolve() if not Path(target).is_absolute() else Path(target).resolve()
        return [path] if path.suffix == ".rs" and path.exists() else []

    files: set[Path] = set()
    for cmd in (
        ["git", "diff", "--name-only", "HEAD", "--", "crates"],
        ["git", "ls-files", "--others", "--exclude-standard", "--", "crates"],
    ):
        proc = subprocess.run(cmd, cwd=ROOT, text=True, capture_output=True, check=False)
        for raw in proc.stdout.splitlines():
            path = (ROOT / raw).resolve()
            if path.suffix == ".rs" and path.exists():
                files.add(path)
    return sorted(files)


def os_environ(name: str) -> str | None:
    # Isolated for tests and to keep the import list obvious.
    import os

    return os.environ.get(name)


def scan(path: Path) -> list[RustItem]:
    items: list[RustItem] = []
    try:
        lines = path.read_text(errors="replace").splitlines()
    except OSError:
        return items
    for idx, line in enumerate(lines, start=1):
        m = ITEM_RE.match(line)
        if not m:
            continue
        items.append(RustItem(
            name=m.group("name"),
            kind=m.group("kind"),
            path=path.resolve(),
            line=idx,
        ))
    return items


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--audit", action="store_true", help="scan all crate files")
    parser.add_argument("--strict-duplicates", action="store_true",
                        help="fail any duplicate public-ish type name, not only vocabulary names")
    args = parser.parse_args()

    vocab = load_registry()
    all_items = [item for path in rust_files() for item in scan(path)]
    selected_paths = rust_files() if args.audit else changed_files()
    selected_items = [item for path in selected_paths for item in scan(path)]

    by_name: dict[str, list[RustItem]] = {}
    for item in all_items:
        by_name.setdefault(item.name, []).append(item)

    errors: list[str] = []
    warnings: list[str] = []

    for item in selected_items:
        entry = vocab.get(item.name)
        if entry is None:
            if args.strict_duplicates and len(by_name.get(item.name, [])) > 1:
                locations = ", ".join(f"{rel(i.path)}:{i.line}" for i in by_name[item.name])
                errors.append(f"duplicate public type `{item.name}`: {locations}")
            continue

        owner_match = item.path == entry.owner
        same_kind = entry.kind == "*" or item.kind == entry.kind
        if not owner_match and same_kind:
            msg = (
                f"{rel(item.path)}:{item.line}: `{item.kind} {item.name}` is owned by "
                f"{rel(entry.owner)} ({entry.mode})"
            )
            if entry.mode == "enforce":
                errors.append(msg)
            else:
                warnings.append(msg)

    if args.audit:
        for name, entry in vocab.items():
            defs = by_name.get(name, [])
            wrong = [
                item for item in defs
                if item.path != entry.owner and (entry.kind == "*" or item.kind == entry.kind)
            ]
            if wrong:
                locations = ", ".join(f"{rel(i.path)}:{i.line}" for i in wrong)
                msg = f"`{name}` also defined outside owner {rel(entry.owner)}: {locations}"
                if entry.mode == "enforce":
                    errors.append(msg)
                else:
                    warnings.append(msg)

    for msg in warnings:
        print(f"[type-vocabulary] WARN: {msg}", file=sys.stderr)
    for msg in errors:
        print(f"[type-vocabulary] FAIL: {msg}", file=sys.stderr)

    if errors:
        _emit_remediation_block(selected_items, vocab)
        return 1
    return 0


def _emit_remediation_block(
    selected_items: list[RustItem],
    vocab: dict[str, VocabularyEntry],
) -> None:
    """Per-violation remediation: dep status + suggested edits, not a one-liner."""
    violations: list[tuple[RustItem, VocabularyEntry]] = []
    for item in selected_items:
        entry = vocab.get(item.name)
        if entry is None:
            continue
        if item.path == entry.owner:
            continue
        if entry.kind != "*" and item.kind != entry.kind:
            continue
        if entry.mode != "enforce":
            continue
        violations.append((item, entry))

    if not violations:
        return

    print("", file=sys.stderr)
    print("[type-vocabulary] How to fix each violation:", file=sys.stderr)
    print("", file=sys.stderr)

    for item, entry in violations:
        offender_crate = _crate_of(item.path)
        owner_crate = _crate_of(entry.owner)
        owner_mod = _module_path(entry.owner)
        canonical_use = f"pub use {owner_mod}::{item.name};"
        dep_present = _crate_depends_on(offender_crate, owner_crate) if offender_crate and owner_crate else None

        print(f"  {rel(item.path)}:{item.line}", file=sys.stderr)
        print(f"    Offender: `{item.kind} {item.name}` in crate `{offender_crate or '(unknown)'}`", file=sys.stderr)
        print(f"    Canonical owner: `{owner_crate or '(unknown)'}` at {rel(entry.owner)}", file=sys.stderr)
        if dep_present is True:
            print(f"    Dep check: {offender_crate}/Cargo.toml ALREADY declares {owner_crate}.", file=sys.stderr)
            print(f"    Fix: replace the local definition with:", file=sys.stderr)
            print(f"        {canonical_use}", file=sys.stderr)
            print(f"    Then reconcile any signature mismatches that surface.", file=sys.stderr)
        elif dep_present is False:
            print(f"    Dep check: {offender_crate}/Cargo.toml DOES NOT declare {owner_crate}. Add first:", file=sys.stderr)
            print(f"        # in crates/{offender_crate}/Cargo.toml under [dependencies]", file=sys.stderr)
            print(f"        {owner_crate}.workspace = true", file=sys.stderr)
            print(f"    Then replace the local definition with:", file=sys.stderr)
            print(f"        {canonical_use}", file=sys.stderr)
            print(f"    Then reconcile any signature mismatches that surface.", file=sys.stderr)
        else:
            print(f"    Fix: replace the local definition with `{canonical_use}` (verify Cargo.toml deps yourself).", file=sys.stderr)
        if entry.notes:
            print(f"    Notes from registry: {entry.notes}", file=sys.stderr)
        print("", file=sys.stderr)

    print("[type-vocabulary] If the duplication is intentional (test helper, deliberate divergence),", file=sys.stderr)
    print("  have an architect add an exception to harness/type-vocabulary.tsv with mode=audit.", file=sys.stderr)
    print(f"  Registry is at: {rel(REGISTRY)}", file=sys.stderr)


def _crate_of(path: Path) -> str | None:
    """Extract the crate name from a crates/<name>/src/... path."""
    parts = path.resolve().parts
    if "crates" not in parts:
        return None
    idx = parts.index("crates")
    if idx + 1 >= len(parts):
        return None
    return parts[idx + 1]


def _module_path(path: Path) -> str:
    """Convert crates/<crate>/src/foo/bar.rs to <crate_underscored>::foo::bar.
    Treats lib.rs / mod.rs as the parent module."""
    crate = _crate_of(path)
    if crate is None:
        return "unknown"
    crate_mod = crate.replace("-", "_")
    parts = path.resolve().parts
    src_idx = parts.index("src")
    after_src = list(parts[src_idx + 1:])
    if after_src and after_src[-1] in {"lib.rs", "mod.rs"}:
        after_src = after_src[:-1]
    elif after_src:
        after_src[-1] = after_src[-1].removesuffix(".rs")
    if not after_src:
        return crate_mod
    return crate_mod + "::" + "::".join(after_src)


def _crate_depends_on(offender: str, owner: str) -> bool | None:
    """Check if offender's Cargo.toml lists owner under [dependencies].
    Returns None on read failure."""
    cargo = ROOT / "crates" / offender / "Cargo.toml"
    try:
        text = cargo.read_text()
    except OSError:
        return None
    in_deps = False
    for raw in text.splitlines():
        line = raw.strip()
        if line.startswith("[") and line.endswith("]"):
            in_deps = line in {"[dependencies]", "[dev-dependencies]", "[build-dependencies]"}
            continue
        if in_deps and line.startswith(f"{owner} ") or line.startswith(f"{owner}="):
            return True
        if in_deps and line.startswith(owner) and "=" in line:
            head = line.split("=", 1)[0].strip()
            if head == owner:
                return True
    return False


if __name__ == "__main__":
    raise SystemExit(main())
