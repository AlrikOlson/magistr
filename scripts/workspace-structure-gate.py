#!/usr/bin/env python3
"""Ratchet every retained/native Rust file using the shared lexical measure."""

import importlib.util
import json
from pathlib import Path
import sys


REVISION = "2a2430bbcd5dcbed00920f92c6f3e6928f387d95"
BASELINE = Path("subsystems/workspace/STRUCTURE_BASELINE.json")
ROOTS = (Path("crates/magistr-workspace"), Path("subsystems/workspace/reference"))
spec = importlib.util.spec_from_file_location(
    "workflow_structure", Path(__file__).with_name("workflow-structure-gate.py"))
shared = importlib.util.module_from_spec(spec)
spec.loader.exec_module(shared)
measure = shared.measure
LIMITS = shared.LIMITS


def measurements(root):
    # target/ is compiler output, never an exemption for imported source.
    files = [path for subtree in ROOTS for path in (root / subtree).rglob("*.rs")
             if "target" not in path.relative_to(root / subtree).parts
             and "node_modules" not in path.relative_to(root / subtree).parts]
    return {path.relative_to(root).as_posix(): measure(path.read_text(encoding="utf-8")) for path in sorted(files)}


def check(root):
    baseline = json.loads((root / BASELINE).read_text(encoding="utf-8"))
    if baseline.get("version") != 1 or baseline.get("upstream_revision") != REVISION:
        return ["unknown baseline schema or upstream revision"]
    errors = []
    for file, current in measurements(root).items():
        previous = baseline["files"].get(file, {})
        for metric, value in current.items():
            limit = max(LIMITS[metric], previous.get(metric, 0))
            if value > limit:
                errors.append(f"{file}: {metric} grew to {value} (maximum {limit})")
    return errors


if __name__ == "__main__":
    try:
        failures = check(Path(sys.argv[1]).resolve())
    except (OSError, ValueError, KeyError) as error:
        failures = [str(error)]
    for failure in failures:
        print(f"ERROR: workspace-structure — {failure}", file=sys.stderr)
    if failures:
        sys.exit(1)
    print("OK: native and reference Rust remain within reviewed per-file structure limits")
