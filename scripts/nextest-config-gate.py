#!/usr/bin/env python3
"""Validate nextest's real configuration parser without rebuilding tests."""
import argparse
import json
from pathlib import Path
import subprocess
import tempfile
import tomllib

ROOT = Path(__file__).resolve().parent.parent


def command(args, root):
    result = subprocess.run(args, cwd=root, text=True, capture_output=True)
    output = result.stdout + result.stderr
    if result.returncode or "unknown configuration key" in output:
        raise RuntimeError(f"{' '.join(map(str, args))}\n{output}")
    return result.stdout


def profiles(config):
    data = tomllib.loads(config.read_text())
    return [name for name, settings in data.get("profile", {}).items()
            if "default-filter" in settings]


def validate_syntax(root, config):
    # nextest accepts recorded Cargo/binary metadata. An empty binary inventory
    # invokes its normal TOML, key and filter parser without compiling or
    # pretending to prove which real tests a default-filter selects.
    metadata = command(["cargo", "metadata", "--format-version", "1", "--no-deps", "--locked"], root)
    version = command(["rustc", "-vV"], root)
    host = next(line.removeprefix("host: ") for line in version.splitlines() if line.startswith("host: "))
    libdir = command(["rustc", "--print", "target-libdir"], root).strip()
    inventory = {"rust-build-meta": {
        "target-directory": json.loads(metadata)["target_directory"],
        "base-output-directories": [], "non-test-binaries": {}, "linked-paths": [],
        "platforms": {"host": {"platform": {"triple": host, "target-features": "unknown"},
                               "libdir": {"status": "available", "path": libdir}}, "targets": []}},
        "rust-binaries": {}}
    with tempfile.TemporaryDirectory() as directory:
        cargo = Path(directory) / "cargo.json"
        binaries = Path(directory) / "binaries.json"
        cargo.write_text(metadata)
        binaries.write_text(json.dumps(inventory))
        command(["cargo", "nextest", "list", "--cargo-metadata", str(cargo),
                 "--binaries-metadata", str(binaries), "--config-file", str(config),
                 "--message-format", "json"], root)


def selected_count(data):
    return sum(test.get("filter-match", {}).get("status") == "matches"
               for suite in data.get("rust-suites", {}).values()
               for test in suite.get("testcases", {}).values())


def validate_filters(root, config, archive):
    filtered = profiles(config)
    if filtered and (archive is None or not archive.is_file()):
        raise RuntimeError("Profile default-filters require a compiled nextest archive: "
                           "pass --archive <path> after cargo nextest archive. No selection check ran.")
    for profile in filtered:
        with tempfile.TemporaryDirectory() as directory:
            output = command(["cargo", "nextest", "list", "--archive-file", str(archive.resolve()),
                              "--extract-to", directory, "--workspace-remap", str(root),
                              "--config-file", str(config), "-P", profile, "--run-ignored", "all",
                              "--message-format", "json"], root)
        count = selected_count(json.loads(output))
        if not count:
            raise RuntimeError(f"Profile '{profile}' has a default-filter that selects no test")
        print(f"OK: profile '{profile}' selects {count} tests from the compiled archive")
    return len(filtered)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, default=ROOT)
    parser.add_argument("--config", type=Path)
    parser.add_argument("--archive", type=Path)
    mode = parser.add_mutually_exclusive_group()
    mode.add_argument("--syntax-only", action="store_true")
    mode.add_argument("--filters-only", action="store_true")
    args = parser.parse_args()
    root = args.root.resolve()
    config = (args.config or root / ".config/nextest.toml").resolve()
    try:
        if not args.filters_only:
            validate_syntax(root, config)
            print("OK: nextest parsed every configuration key and filter without compiling tests")
        if not args.syntax_only and not validate_filters(root, config, args.archive):
            print("OK: no profile declares a default-filter")
    except (OSError, ValueError, RuntimeError, StopIteration) as error:
        print(f"FAILED: {error}")
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
