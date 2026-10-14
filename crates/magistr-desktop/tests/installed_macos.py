#!/usr/bin/env python3
"""Verify a copied native app opens through Launch Services without build tools."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import plistlib
import shutil
import subprocess
import tempfile
import time

from native_window import ROOT, SOURCE, run


def pids(binary):
    rows = run("ps", "-axo", "pid=,comm=").stdout.splitlines()
    return {int(row.strip().split(None, 1)[0]) for row in rows
            if len(row.strip().split(None, 1)) == 2
            and row.strip().split(None, 1)[1] == str(binary)}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("app", type=Path)
    parser.add_argument("--evidence", type=Path, default=ROOT / ".magistr/desktop-native/installed")
    args = parser.parse_args()
    evidence = args.evidence.resolve()
    evidence.mkdir(parents=True, exist_ok=True)
    driver = evidence / "driver"
    run("swiftc", "-parse-as-library", *(SOURCE / "tests" / file for file in (
        "native_window.swift", "rich_conversation.swift", "attachment.swift",
        "draft_persistence.swift")), "-o", driver, timeout=60)

    observer_environment = dict(os.environ)

    def call(mode, *values):
        try:
            result = run(driver, mode, *values, env=observer_environment)
        except subprocess.CalledProcessError as error:
            raise AssertionError(error.stderr) from error
        return json.loads(result.stdout) if result.stdout.strip() else None

    environment = call("environment")
    assert environment["accessibility"] and environment["screen_capture"], environment
    with tempfile.TemporaryDirectory(prefix="Magistr installed app ") as temporary:
        directory = Path(temporary).resolve()
        app = directory / "Magistr.app"
        observer_environment["MAGISTR_OBSERVER_APP"] = str(app)
        shutil.copytree(args.app.resolve(), app, symlinks=True)
        run("codesign", "--verify", "--deep", "--strict", app)
        info = plistlib.loads((app / "Contents/Info.plist").read_bytes())
        assert info["CFBundleIdentifier"] == "dev.magistr.desktop", info
        binary = app / "Contents/MacOS" / info["CFBundleExecutable"]
        log = evidence / "launch.log"
        arguments = ["/usr/bin/open", "-n", "-W", "-a", str(app),
                     "--stdout", str(log), "--stderr", str(log)]
        for key, value in {
            "PATH": "/usr/bin:/bin:/usr/sbin:/sbin",
            "XDG_CONFIG_HOME": str(directory / "config"),
            "XDG_DATA_HOME": str(directory / "data"),
            "MAGISTR_HOME": str(directory / "state"),
        }.items():
            arguments.extend(["--env", key + "=" + value])
        arguments.extend(["--args", "--reduce-motion"])
        opener = subprocess.Popen(arguments, cwd=directory)
        pid = None
        try:
            deadline = time.monotonic() + 20
            while time.monotonic() < deadline:
                found = pids(binary)
                assert len(found) <= 1, found
                if found:
                    pid = found.pop()
                    break
                if opener.poll() is not None:
                    raise AssertionError("App did not stay open: " + log.read_text())
                time.sleep(0.1)
            assert pid is not None, "Installed app did not start"
            call("activate", pid)
            deadline = time.monotonic() + 20
            while time.monotonic() < deadline:
                tree = call("ax", pid)
                if any(row.get("AXRole") == "AXButton" and "Open project" in str(row)
                       for row in tree):
                    break
                time.sleep(0.1)
            else:
                raise AssertionError("Project chooser was not accessible: " + json.dumps(tree))
            call("ax", pid, evidence / "accessibility.json")
            call("screenshot", pid, evidence / "chooser.png")
            call("quit", pid)
            assert opener.wait(timeout=15) == 0
            assert not pids(binary), "App stayed alive after native Quit"
            (evidence / "receipt.json").write_text(json.dumps({
                "checks": {"bundle_signature": True, "launch_services": True,
                           "no_build_tools": True, "accessible_chooser": True, "native_quit": True},
                "binary_sha256": hashlib.sha256(binary.read_bytes()).hexdigest(),
                "bundle_identifier": info["CFBundleIdentifier"],
                "source_app": str(args.app.resolve()),
            }, indent=2) + "\n")
        finally:
            if pid is not None and pid in pids(binary):
                os.kill(pid, 15)
            if opener.poll() is None:
                opener.terminate()
                opener.wait(timeout=10)
            call("restore-front", environment["front_pid"])
    print("PASS: copied app signature, Launch Services, accessible chooser without build tools, native Quit")


if __name__ == "__main__":
    main()
