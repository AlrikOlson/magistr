#!/usr/bin/env python3
"""Execute one CI stage with a real deadline and a compact, source-bound timing receipt."""
import argparse
import json
import os
from pathlib import Path
import signal
import subprocess
import time
import xml.etree.ElementTree as ET


def slow_tests(path, limit=20):
    if not path.exists():
        return []
    root = ET.parse(path).getroot()
    return sorted(
        ({"name": f"{case.get('classname', '')}::{case.get('name', '')}",
          "seconds": float(case.get("time", "0"))} for case in root.iter("testcase")),
        key=lambda item: item["seconds"], reverse=True)[:limit]


def run(command, budget):
    started = time.monotonic()
    child = subprocess.Popen(command, start_new_session=True)
    timed_out = False
    try:
        code = child.wait(timeout=budget)
    except subprocess.TimeoutExpired:
        timed_out = True
        os.killpg(child.pid, signal.SIGTERM)
        try:
            child.wait(timeout=5)
        except subprocess.TimeoutExpired:
            os.killpg(child.pid, signal.SIGKILL)
            child.wait()
        code = 124
    return {"seconds": round(time.monotonic() - started, 3), "exit_code": code,
            "budget_seconds": budget, "timed_out": timed_out}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--name", required=True)
    parser.add_argument("--budget", type=float, required=True)
    parser.add_argument("--junit", type=Path)
    parser.add_argument("command", nargs=argparse.REMAINDER)
    args = parser.parse_args()
    command = args.command[1:] if args.command[:1] == ["--"] else args.command
    if not command or args.budget <= 0 or Path(args.name).name != args.name:
        parser.error("A command, positive budget and simple stage name are required")
    receipt = run(command, args.budget)
    receipt.update({"name": args.name, "source": os.environ.get("GITHUB_SHA"),
                    "run_id": os.environ.get("GITHUB_RUN_ID")})
    if args.junit:
        receipt["slow_tests"] = slow_tests(args.junit)
    directory = Path("target/ci-timings")
    directory.mkdir(parents=True, exist_ok=True)
    (directory / f"{args.name}.json").write_text(json.dumps(receipt, indent=2) + "\n")
    print(json.dumps(receipt))
    if os.environ.get("GITHUB_STEP_SUMMARY"):
        with open(os.environ["GITHUB_STEP_SUMMARY"], "a") as stream:
            stream.write(f"| {args.name} | {receipt['seconds']:.1f}s | {args.budget:g}s | "
                         f"{'passed' if receipt['exit_code'] == 0 else 'failed'} |\n")
    raise SystemExit(receipt["exit_code"])


if __name__ == "__main__":
    main()
