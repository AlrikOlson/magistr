#!/usr/bin/env python3
"""Cohesive modules, companion tests, dependency direction, function length and lint hygiene.

The reviewed allowlists only shrink. --generate reports a possible baseline;
it never changes an allowlist. Imported Rust retains its per-file ratchets.
"""
import os
from pathlib import Path
import subprocess
import sys
from source_scan import SourceTree, hits, pattern

OWNED = tuple(f"crates/{name}/src/" for name in (
    "magistr-cli", "magistr-engine", "magistr-domain", "magistr-application",
    "magistr-workflow-contracts"))
IMPORTED = ("crates/magistr-workspace/", "crates/magistr-workflow/")


def cohesive(tree, relative, text):
    count = len(pattern(r"^pub (struct|enum|fn|trait|type|const|static|async fn)").findall(text))
    if count <= 1:
        return []
    path = Path(relative)
    name, directory = path.stem, path.parent
    prefix = f"ERROR: {relative} exports {count} public items"
    if name in ("lib", "main"):
        return [f"ERROR: {relative} is a crate root exporting {count} public items; move them behind a private module it re-exports"]
    tests = path.with_suffix(".tests.rs")
    if name == "mod":
        name, directory = directory.name, directory.parent
    parent = None
    for candidate in (directory / "mod.rs", directory.with_suffix(".rs"),
                      directory / "lib.rs", directory / "main.rs"):
        if pattern(rf"^\s*(pub(\([a-z]+\))? )?mod {name};").search(tree.read(candidate)):
            parent = candidate
            break
    errors = []
    if parent is None:
        errors.append(f"{prefix} but no parent facade declares `mod {name};`")
    elif pattern(rf"^\s*pub mod {name};").search(tree.read(parent)):
        errors.append(f"{prefix} through a public module path (`pub mod {name};` in {parent}); declare it privately and re-export it")
    elif not pattern(rf"^\s*pub(\([a-z]+\))? use (self::)?{name}::").search(tree.read(parent)):
        errors.append(f"{prefix} but {parent} re-exports none of them (`pub use {name}::…`)")
    first = next((line for line in text.splitlines() if line.strip()), "")
    if not first.startswith("//!"):
        errors.append(f"{prefix} without a `//!` module doc naming what the items decide together and why they change together")
    if not (tree.root / tests).is_file():
        errors.append(f"{prefix} without its companion tests {tests}")
    return errors


def long_functions(text):
    # Preserve the existing gate's line-based measurement while removing awk
    # startup for each file. Imported crates use their stricter lexical ratchet.
    lines = text.splitlines()
    start = 0
    header = pattern(r"^\s*(pub(\(crate\))? )?(async )?fn [a-z_]")
    while start < len(lines):
        if not header.search(lines[start]):
            start += 1
            continue
        name = lines[start].split("fn ", 1)[1].split("(", 1)[0].split("<", 1)[0]
        end, depth = start, 0
        while True:
            end += 1
            if end >= len(lines):
                break
            depth += lines[end].count("{") - lines[end].count("}")
            if depth <= 0 or end >= start + 500:
                break
        if end - start > 150:
            yield start + 1, name, end - start
        start = end + 1


def check(tree, generate=False):
    sources = tree.rust()
    owned = {name: text for name, text in sources.items()
             if name.startswith(OWNED) and not name.endswith(".tests.rs")}
    if generate:
        print("Files a fresh baseline would hold (the checked-in allowlist is not rewritten):")
        for name, text in sorted(owned.items()):
            if cohesive(tree, name, text) or "mod tests {" in text:
                print(name)
        return 0
    errors = []
    if (tree.root / "crates/magistr-workspace").is_dir():
        result = subprocess.run([sys.executable, "-B", str(tree.root / "scripts/workspace-structure-gate.py"),
                                 str(tree.root)])
        if result.returncode:
            errors.append("ERROR: workspace structure ratchet failed")
    allowed = tree.allowlist("lint-structure-allowlist.txt")
    dependency_allowed = tree.allowlist("lint-dep-direction-allowlist.txt")
    length_allowed = tree.allowlist("lint-fn-length-allowlist.txt")
    for name, text in owned.items():
        if name not in allowed:
            errors.extend(cohesive(tree, name, text))
            if "#[cfg(test)]" in text and "mod tests {" in text:
                errors.append(f"ERROR: {name} has inline #[cfg(test)] mod tests — move tests to {name[:-3]}.tests.rs")
    directions = {"core": ("detection", "agent", "orchestration"),
                  "detection": ("agent", "orchestration"), "agent": ("orchestration",)}
    for name, text in sources.items():
        if not name.endswith(".tests.rs"):
            if name not in dependency_allowed:
                for domain, forbidden in directions.items():
                    if name.startswith(f"crates/magistr-cli/src/{domain}/"):
                        for target in forbidden:
                            errors.extend(f"ERROR: {name} — forbidden import from {domain} → {target}: {hit}"
                                          for hit in hits(text, rf"use crate::{target}::"))
            if not name.startswith(IMPORTED):
                for line, function, length in long_functions(text):
                    if f"{name}:{function}" not in length_allowed:
                        errors.append(f"ERROR: {name}:{line} — function '{function}' is {length} lines (max 150)")
        if name.startswith(("crates/magistr-cockpit/", "crates/magistr-desktop/")):
            errors.extend(f"ERROR: {name} — the crate denies dead/unused code; delete it instead of silencing the lint: {hit}"
                          for hit in hits(text, r"#!?\[(allow|expect)\([^]]*(dead_code|unused)"))
    if errors:
        print("\n".join(errors))
        print(f"FAILED: {len(errors)} structure violation(s); reviewed baselines are pruned, not extended")
        return 1
    print("OK: structure lint passed")
    return 0


if __name__ == "__main__":
    root = os.environ.get("LINT_PROJECT_ROOT") or Path(__file__).resolve().parent.parent
    raise SystemExit(check(SourceTree(root), "--generate" in sys.argv[1:]))
