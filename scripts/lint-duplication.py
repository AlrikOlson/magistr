#!/usr/bin/env python3
"""Reject identical files and cross-tree Rust clones with the locked jscpd tool."""
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile


def canonical(path):
    return Path(str(path).removeprefix("\\\\?\\")).resolve()


def main():
    scripts = Path(__file__).resolve().parent
    root = canonical(os.environ.get("LINT_PROJECT_ROOT", scripts.parent))
    owner = root / os.environ.get("LINT_DUP_OWNER", "crates/magistr-cli/src/application")
    surface = root / os.environ.get("LINT_DUP_SURFACE", "crates/magistr-desktop/src")
    executable = os.environ.get("JSCPD", str(scripts / "node_modules/.bin/jscpd"))
    if not shutil.which(executable):
        print("ERROR: jscpd is unavailable — run npm ci --prefix scripts from the repository root")
        return 1
    for label, directory in (("owner", owner), ("desktop", surface)):
        if not directory.is_dir():
            print(f"ERROR: missing {label} tree at {directory.relative_to(root)}")
            return 1
    errors, hashes = [], {}
    for file in sorted(owner.rglob("*.rs")):
        hashes.setdefault(hashlib.sha256(file.read_bytes()).digest(), file)
    for file in sorted(surface.rglob("*.rs")):
        twin = hashes.get(hashlib.sha256(file.read_bytes()).digest())
        if twin:
            errors.append(f"ERROR: {file.relative_to(root)} is byte-identical to {twin.relative_to(root)} — the desktop must not copy the owner tree")
    with tempfile.TemporaryDirectory() as directory:
        result = subprocess.run([executable, "--min-tokens", os.environ.get("LINT_DUP_MIN_TOKENS", "50"),
                                 "--format", "rust", "--reporters", "json", "--output", directory,
                                 "--silent", "--absolute", "--no-gitignore", str(owner), str(surface)],
                                text=True, capture_output=True)
        if result.returncode:
            print(f"ERROR: jscpd did not run (executable: {executable})\n{result.stdout}{result.stderr}")
            return 1
        report = Path(directory) / "jscpd-report.json"
        if not report.is_file():
            print(f"ERROR: jscpd wrote no report to {directory}")
            return 1
        for duplicate in json.loads(report.read_text()).get("duplicates", []):
            first, second = duplicate["firstFile"], duplicate["secondFile"]
            a, b = canonical(first["name"]), canonical(second["name"])
            if ((a.is_relative_to(owner) and b.is_relative_to(surface))
                    or (b.is_relative_to(owner) and a.is_relative_to(surface))):
                errors.append(f"ERROR: clone — {a.relative_to(root)}:{first['start']}-{first['end']} duplicates "
                              f"{b.relative_to(root)}:{second['start']}-{second['end']} ({duplicate.get('tokens', '?')} tokens)")
    if errors:
        print("\n".join(errors))
        print(f"FAILED: {len(errors)} duplication violation(s); move shared behavior to its application owner")
        return 1
    print(f"OK: no code shared between {owner.relative_to(root)} and {surface.relative_to(root)}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
