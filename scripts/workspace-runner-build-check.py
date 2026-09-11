#!/usr/bin/env python3
"""Rebuild the preserved Go runner in a disposable reference workspace.

Only the Cargo workspace inputs and runner plugin are copied. Compiler output
and the rebuilt component stay in the temporary directory; the checked-in
guest is read only. A different rebuilt hash is reported without claiming
that the original prebuilt guest has a reproducible build attestation.
"""

import hashlib
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile


REFERENCE = Path("subsystems/workspace/reference")
RUNNER = Path("plugins/runner-go")
COMPONENT_HEADER = b"\x00asm\x0d\x00\x01\x00"


def check(root):
    reference = root / REFERENCE
    original = reference / RUNNER / "guest.wasm"
    pinned = hashlib.sha256(original.read_bytes()).hexdigest()
    ignore = shutil.ignore_patterns("target", "artifacts", "__pycache__")
    with tempfile.TemporaryDirectory(prefix="magistr-runner-build-") as directory:
        temporary = Path(directory)
        copied = temporary / "reference"
        copied.mkdir()
        for filename in ("Cargo.toml", "Cargo.lock", "rust-toolchain.toml"):
            shutil.copy2(reference / filename, copied / filename)
        shutil.copytree(reference / "crates", copied / "crates", ignore=ignore)
        shutil.copytree(reference / RUNNER, copied / RUNNER, ignore=ignore)

        environment = os.environ.copy()
        # Componentizer output stays disposable. The runner builder explicitly
        # overrides this path for the guest and must use that guest output.
        environment["CARGO_TARGET_DIR"] = str(temporary / "host-target")
        command = ["bash", str(copied / RUNNER / "build.sh")]
        print("Building runner-go with locked dependencies in a temporary copy", flush=True)
        subprocess.run(command, cwd=temporary, env=environment, check=True)

        rebuilt = (copied / RUNNER / "guest.wasm").read_bytes()
        if not rebuilt.startswith(COMPONENT_HEADER):
            raise ValueError("runner builder did not produce a WebAssembly component")
        digest = hashlib.sha256(rebuilt).hexdigest()
        print(f"Pinned runner SHA-256:  {pinned}")
        print(f"Rebuilt runner SHA-256: {digest} ({len(rebuilt)} bytes)")
        print(f"Byte-identical to pinned guest: {str(digest == pinned).lower()}")

    if hashlib.sha256(original.read_bytes()).hexdigest() != pinned:
        raise ValueError("checked-in runner changed during the disposable build check")
    print("OK: runner component rebuilt; temporary outputs cleaned; pinned guest unchanged")


if __name__ == "__main__":
    if len(sys.argv) > 2:
        print("usage: workspace-runner-build-check.py [project-root]", file=sys.stderr)
        sys.exit(2)
    project = Path(sys.argv[1]).resolve() if len(sys.argv) == 2 else Path(__file__).resolve().parents[1]
    try:
        check(project)
    except subprocess.CalledProcessError as error:
        print(f"ERROR: workspace-runner-build — builder exited {error.returncode}", file=sys.stderr)
        sys.exit(error.returncode if error.returncode > 0 else 1)
    except (OSError, ValueError) as error:
        print(f"ERROR: workspace-runner-build — {error}", file=sys.stderr)
        sys.exit(1)
