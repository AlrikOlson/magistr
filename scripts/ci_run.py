#!/usr/bin/env python3
"""Run the Rust selection produced by ci_plan without shell interpolation."""
import argparse
import json
import os
from pathlib import Path
import re
import subprocess
import sys

ROOT = Path(__file__).resolve().parent.parent


def run(*args):
    subprocess.run(list(args), cwd=ROOT, check=True)


def packages():
    selected = json.loads(os.environ["CI_PACKAGES"])
    if not selected or any(not re.fullmatch(r"[A-Za-z0-9_-]+", name) for name in selected):
        raise ValueError("Expected a nonempty Cargo package selection")
    return [arg for name in selected for arg in ("-p", name)]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("stage", choices=["rust-build", "rust-test", "contracts"])
    parser.add_argument("--partition", default="1/1")
    args = parser.parse_args()
    if args.stage == "rust-build":
        selected = packages()
        run("cargo", "fmt", "--all", "--", "--check")
        # Clippy performs the all-target type check and lint in one compilation pass.
        run("cargo", "clippy", *selected, "--all-targets", "--locked", "--", "-D", "warnings")
        run("cargo", "nextest", "archive", *selected, "--locked",
            "--archive-file", "target/ci-tests.tar.zst")
    elif args.stage == "rust-test":
        if not re.fullmatch(r"[1-9][0-9]*/[1-9][0-9]*", args.partition):
            raise ValueError("Invalid test partition")
        run("cargo", "nextest", "run", "--archive-file", "target/ci-tests.tar.zst",
            "--extract-to", str(ROOT), "--extract-overwrite", "--workspace-remap", str(ROOT),
            "--profile", "diagnostic" if os.environ.get("GITHUB_EVENT_NAME") in
            ("schedule", "workflow_dispatch") else "ci", "--partition", f"hash:{args.partition}")
    else:
        run(sys.executable, "-B", "scripts/version.py", "check")
        for name in ("version", "cli-release", "cargo_publication", "public_link_check",
                     "release-security", "agent-mcp-eval"):
            run(sys.executable, "-B", f"scripts/{name}.tests.py")


if __name__ == "__main__":
    main()
