#!/usr/bin/env python3
"""Regression checks for release source selection and artifact identity."""
import hashlib
import importlib.util
import io
import json
from pathlib import Path
import tarfile
import tempfile
import unittest

spec = importlib.util.spec_from_file_location("cli_release", Path(__file__).with_name("cli-release.py"))
release = importlib.util.module_from_spec(spec)
spec.loader.exec_module(release)
SOURCE = "1" * 40


def archive(path, files):
    with tarfile.open(path, "w:gz") as target:
        for name, content in files.items():
            data = content if isinstance(content, bytes) else json.dumps(content).encode()
            item = tarfile.TarInfo(name)
            item.size = len(data)
            target.addfile(item, io.BytesIO(data))


def write_json(path, content):
    path.write_text(json.dumps(content))


class HostedRunTests(unittest.TestCase):
    def test_only_successful_manual_packages_from_main_are_selected(self):
        run = {"repository": {"full_name": "owner/repo"}, "path": ".github/workflows/cli-packages.yml",
               "event": "workflow_dispatch", "head_branch": "main", "status": "completed",
               "conclusion": "success", "head_sha": SOURCE}
        self.assertEqual(release.check_run(run, "owner/repo"), SOURCE)
        for changes in ({"repository": {"full_name": "someone/else"}},
                        {"path": ".github/workflows/unrelated.yml"}, {"head_branch": "topic"},
                        {"event": "pull_request"}, {"conclusion": "failure"}, {"status": "in_progress"}):
            with self.subTest(changes=changes), self.assertRaises(ValueError):
                release.check_run(dict(run, **changes), "owner/repo")

    def test_a_previous_green_run_cannot_override_a_later_failure_or_another_revision(self):
        run = {"head_sha": SOURCE, "event": "push", "status": "completed", "conclusion": "success",
               "run_number": 1}
        release.check_quality([run], SOURCE)
        for runs in ([], [dict(run, head_sha="2" * 40)],
                     [run, dict(run, run_number=2, conclusion="failure")],
                     [run, dict(run, run_number=2, status="in_progress", conclusion=None)]):
            with self.subTest(runs=runs), self.assertRaises(ValueError):
                release.check_quality(runs, SOURCE)


class ArtifactTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        crate = self.root / "magistr-cli-0.1.0.crate"
        archive(crate, {"magistr-cli-0.1.0/.cargo_vcs_info.json": {"git": {"sha1": SOURCE}}})
        cargo = [{"package": "magistr-cli", "version": "0.1.0", "file": crate.name,
                  "sha256": release.digest(crate)}]
        native = b"verified native binary fixture"
        npm = self.root / "magistr-0.1.0.tgz"
        archive(npm, {"package/package.json": {"name": "magistr", "version": "0.1.0",
                                             "os": ["darwin"], "cpu": ["arm64"]},
                      "package/bin/magistr": native})
        write_json(self.root / "cargo-inventory.json", cargo)
        (self.root / "publication-order.txt").write_text("magistr-cli\n")
        write_json(self.root / "npm-inventory.json",
                   {"name": "magistr", "version": "0.1.0", "filename": npm.name})
        write_json(self.root / "binary-inventory.json",
                   {"version": "0.1.0", "target": "aarch64-apple-darwin",
                    "binarySha256": hashlib.sha256(native).hexdigest()})
        (self.root / "SHA256SUMS").write_text(
            f"{release.digest(crate)}  {crate.name}\n{release.digest(npm)}  {npm.name}\n")
        write_json(self.root / "cargo-install-verification.json",
                   {"passed": True, "version": "0.1.0", "profile": "release", "checkoutRequired": False,
                    "artifacts": {crate.name: release.digest(crate)}})
        write_json(self.root / "npm-install-verification.json",
                   {"passed": True, "version": "0.1.0", "artifactSha256": release.digest(npm),
                    "rustOrBunOnPath": False, "globalInstall": True, "npxInit": True,
                    "pinnedConfigFreshCache": True, "nativeMcpAndWorker": True, "sigterm": True,
                    "removal": True, "usageExitCode": 2})

    def test_reviewed_archives_and_matching_receipts_are_accepted(self):
        self.assertEqual(release.verify(self.root, SOURCE)["version"], "0.1.0")

    def test_corrupt_archive_or_wrong_source_revision_is_refused(self):
        with self.assertRaisesRegex(ValueError, "release revision"):
            release.verify(self.root, "2" * 40)
        (self.root / "magistr-cli-0.1.0.crate").write_bytes(b"changed")
        with self.assertRaisesRegex(ValueError, "checksum"):
            release.verify(self.root, SOURCE)

    def test_debug_install_and_different_artifact_receipts_are_refused(self):
        path = self.root / "cargo-install-verification.json"
        original = release.read_json(path)
        for changes in ({"profile": "debug"}, {"artifacts": {}}, {"passed": False}):
            with self.subTest(changes=changes):
                write_json(path, dict(original, **changes))
                with self.assertRaises(ValueError):
                    release.verify(self.root, SOURCE)
        write_json(path, original)
        path = self.root / "npm-install-verification.json"
        receipt = release.read_json(path)
        receipt["artifactSha256"] = "0" * 64
        write_json(path, receipt)
        with self.assertRaisesRegex(ValueError, "different artifacts"):
            release.verify(self.root, SOURCE)

    def test_missing_proof_and_reordered_publication_are_refused(self):
        (self.root / "publication-order.txt").write_text("magistr-other\n")
        with self.assertRaisesRegex(ValueError, "publication order"):
            release.verify(self.root, SOURCE)
        (self.root / "publication-order.txt").write_text("magistr-cli\n")
        (self.root / "npm-install-verification.json").unlink()
        with self.assertRaises(FileNotFoundError):
            release.verify(self.root, SOURCE)


if __name__ == "__main__":
    unittest.main()
