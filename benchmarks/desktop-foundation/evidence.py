#!/usr/bin/env python3
"""Import native measurements, preserving the immutable finalist comparison."""
import argparse
from datetime import datetime, timezone
import hashlib
import json
from pathlib import Path
import shutil
import subprocess

ROOT = Path(__file__).resolve().parent
PROJECT = ROOT.parent.parent
EVIDENCE = PROJECT / "docs/evidence/desktop-foundation"
ASSETS = PROJECT / "docs/assets/desktop-foundation"


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def save(path, value):
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(value, ensure_ascii=False, indent=2) + "\n")


def seal():
    manifest = {}
    for root in (EVIDENCE, ASSETS):
        for path in root.rglob("*"):
            if path.is_file() and path.name != "manifest.json":
                manifest[str(path.relative_to(PROJECT))] = digest(path)
    save(EVIDENCE / "manifest.json", manifest)


def cpu_bounds():
    rows = []
    for path in sorted((EVIDENCE / "performance").glob("*/run-*/results.json")):
        result = json.loads(path.read_text())
        samples = result["process_samples"]
        initial = {row["pid"] for row in samples[0]["processes"]}
        final = {row["pid"] for row in samples[-1]["processes"]}
        seen = {row["pid"] for sample in samples for row in sample["processes"]}
        assert seen <= final, "An exited helper needs separate accounting"
        later = [row for row in samples[-1]["processes"] if row["pid"] not in initial]
        extra = sum(row["cpu_ns"] for row in later) / 1e9 / (result["stream"]["elapsed_ms"] / 1000) * 100
        estimate = result["stream_cpu_percent_one_core"]
        rows.append({
            "source": str(path.relative_to(PROJECT)),
            "backend": result["backend"],
            "recorded_percent_one_core": estimate,
            "conservative_upper_percent_one_core": estimate + extra,
            "late_helpers": later,
        })
    save(EVIDENCE / "cpu-bounds.json", {
        "method": "Add each late helper's entire lifetime CPU counter divided by the producer duration. The original streaming start snapshot was not retained; the helper may already have been included.",
        "idle": "All idle interval PID sets are stable; no birth correction.",
        "rows": rows,
    })


def selected(performance, journeys):
    performance, journeys = Path(performance), Path(journeys)
    target = EVIDENCE / "selected-validation"
    target.mkdir(parents=True, exist_ok=True)
    for name in ("environment.json", "results.json", "typing.json", "latency.json", "motion.json", "accessibility.json"):
        shutil.copy2(performance / name, target / name)
    shutil.copy2(performance / "active/metrics.json", target / "metrics.json")
    destination = EVIDENCE / "journeys/tauri"
    destination.mkdir(parents=True, exist_ok=True)
    for name in ("journeys.json", "changes.json", "multilingual.json", "workspace-picker.json", "roadmap.json"):
        shutil.copy2(journeys / name, destination / name)
    assets = ASSETS / "tauri"
    assets.mkdir(parents=True, exist_ok=True)
    for name in ("dark", "light", "narrow", "multilingual", "workspace-picker", "changes", "roadmap"):
        shutil.copy2(journeys / (name + ".png"), assets / (name + ".png"))
    sources = {}
    for path in sorted(ROOT.rglob("*")):
        if not path.is_file() or any(part in ("target", "results", "__pycache__", "gen") for part in path.relative_to(ROOT).parts):
            continue
        sources[str(path.relative_to(PROJECT))] = digest(path)
    save(target / "build.json", {
        "recorded_utc": datetime.now(timezone.utc).isoformat(),
        "rustc": subprocess.check_output(["rustc", "+1.99.0", "--version", "--verbose"], text=True).strip(),
        "source_sha256": sources,
        "binary_sha256": digest(ROOT / "target/release/magistr-foundation-tauri"),
        "scope": "Selected fixture after the composer correction, formatting and removal of GPUI. The alternating finalist comparison remains immutable.",
    })


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("mode", choices=("seal", "selected"))
    parser.add_argument("--performance", type=Path)
    parser.add_argument("--journeys", type=Path)
    args = parser.parse_args()
    if args.mode == "selected":
        selected(args.performance, args.journeys)
    cpu_bounds()
    seal()
    print("Desktop foundation evidence sealed")


if __name__ == "__main__":
    main()
