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


def stop_group(child, grace=5):
    try:
        os.killpg(child.pid, signal.SIGTERM)
    except ProcessLookupError:
        child.wait()
        return
    deadline = time.monotonic() + grace
    while time.monotonic() < deadline:
        child.poll()
        try:
            os.killpg(child.pid, 0)
        except ProcessLookupError:
            break
        except PermissionError:
            # macOS can refuse signal 0 during the exit/reap transition.
            # Keep waiting; a real failure to terminate still propagates below.
            pass
        time.sleep(0.02)
    try:
        os.killpg(child.pid, signal.SIGKILL)
    except ProcessLookupError:
        pass
    child.wait()


def select_budget(warm, cold, cache_hit):
    """A stage keeps its warm deadline unless the Rust cache missed and a cold deadline exists."""
    is_cold = cache_hit is not None and cache_hit.strip().lower() != "true"
    if is_cold and cold is not None:
        return cold, True
    return warm, is_cold


def run(command, budget):
    started = time.monotonic()
    child = subprocess.Popen(command, start_new_session=True)
    timed_out = False
    try:
        code = child.wait(timeout=budget)
    except subprocess.TimeoutExpired:
        timed_out = True
        stop_group(child)
        code = 124
    except BaseException:
        stop_group(child)
        raise
    return {"seconds": round(time.monotonic() - started, 3), "exit_code": code,
            "budget_seconds": budget, "timed_out": timed_out}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--name", required=True)
    parser.add_argument("--budget", type=float, required=True,
                        help="Deadline in seconds for a run whose Rust cache was restored")
    parser.add_argument("--cold-budget", type=float,
                        help="Deadline when --cache-hit is not 'true' (a cold build)")
    parser.add_argument("--cache-hit", help="The rust-cache cache-hit output; omit for stages without a Rust cache")
    parser.add_argument("--junit", type=Path, action="append", default=[])
    parser.add_argument("command", nargs=argparse.REMAINDER)
    args = parser.parse_args()
    command = args.command[1:] if args.command[:1] == ["--"] else args.command
    if not command or args.budget <= 0 or Path(args.name).name != args.name:
        parser.error("A command, positive budget and simple stage name are required")
    if args.cold_budget is not None and (args.cold_budget <= 0 or args.cache_hit is None):
        parser.error("--cold-budget needs a positive value and the --cache-hit output")
    budget, cold = select_budget(args.budget, args.cold_budget, args.cache_hit)
    receipt = run(command, budget)
    receipt.update({"name": args.name, "cold": cold, "source": os.environ.get("GITHUB_SHA"),
                    "run_id": os.environ.get("GITHUB_RUN_ID")})
    if args.junit:
        receipt["slow_tests"] = sorted(
            (test for path in args.junit for test in slow_tests(path)),
            key=lambda test: test["seconds"], reverse=True)[:20]
    directory = Path("target/ci-timings")
    directory.mkdir(parents=True, exist_ok=True)
    (directory / f"{args.name}.json").write_text(json.dumps(receipt, indent=2) + "\n")
    print(json.dumps(receipt))
    if os.environ.get("GITHUB_STEP_SUMMARY"):
        with open(os.environ["GITHUB_STEP_SUMMARY"], "a") as stream:
            stream.write(f"| {args.name} | {receipt['seconds']:.1f}s | {budget:g}s"
                         f"{' (cold)' if cold else ''} | "
                         f"{'passed' if receipt['exit_code'] == 0 else 'failed'} |\n")
    raise SystemExit(receipt["exit_code"])


if __name__ == "__main__":
    main()
