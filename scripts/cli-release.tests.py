#!/usr/bin/env python3
"""Regression checks for release source selection and artifact identity."""
from datetime import datetime, timezone
import hashlib
import importlib.util
import io
import json
from pathlib import Path
import tarfile
import tempfile
import unittest
from unittest.mock import patch

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
    def test_selector_requires_successful_source_bound_security_run(self):
        run = {"repository": {"full_name": "owner/repo"}, "path": ".github/workflows/cli-packages.yml",
               "event": "workflow_dispatch", "head_branch": "main", "status": "completed",
               "conclusion": "success", "head_sha": SOURCE}
        check = {"head_sha": SOURCE, "event": "push", "status": "completed", "conclusion": "success",
                 "run_number": 1}
        with patch.object(release, "github", side_effect=[run, {"workflow_runs": [check]},
                                                        {"workflow_runs": []}]) as api:
            with self.assertRaisesRegex(ValueError, "release-security"):
                release.select_run("owner/repo", "123")
        self.assertIn("release-security.yml", api.call_args.args[0])


    def test_resume_run_must_be_completed_manual_publication_from_this_repository(self):
        run = {"repository": {"full_name": "owner/repo"}, "path": ".github/workflows/cli-packages.yml",
               "event": "workflow_dispatch", "head_branch": "main", "status": "completed",
               "conclusion": "success", "head_sha": SOURCE}
        check = {"head_sha": SOURCE, "event": "push", "status": "completed", "conclusion": "success",
                 "run_number": 1}
        previous = dict(run, path=".github/workflows/cli-publish.yml", conclusion="failure")
        for change in ({"repository": {"full_name": "another/repo"}},
                       {"path": ".github/workflows/cli-packages.yml"}, {"status": "in_progress"},
                       {"event": "pull_request"}, {"head_branch": "topic"}):
            with self.subTest(change=change):
                with patch.object(release, "github", side_effect=[
                        run, {"workflow_runs": [check]}, {"workflow_runs": [check]},
                        dict(previous, **change)]):
                    with self.assertRaisesRegex(ValueError, "Previous publication"):
                        release.select_run("owner/repo", "123", "456")
        with patch.object(release, "github", side_effect=[
                run, {"workflow_runs": [check]}, {"workflow_runs": [check]}, previous]):
            with patch("sys.stdout", new_callable=io.StringIO) as output:
                release.select_run("owner/repo", "123", "456")
            self.assertEqual(output.getvalue().strip(), SOURCE)

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
        self.build("0.1.0")

    def build(self, version):
        """Write a complete reviewed artifact set for one product version."""
        crate = self.root / f"magistr-cli-{version}.crate"
        archive(crate, {f"magistr-cli-{version}/.cargo_vcs_info.json": {"git": {"sha1": SOURCE}}})
        cargo = [{"package": "magistr-cli", "version": version, "file": crate.name,
                  "sha256": release.digest(crate)}]
        native = b"verified native binary fixture"
        npm = self.root / f"magistr-{version}.tgz"
        archive(npm, {"package/package.json": {"name": "magistr", "version": version,
                                             "os": ["darwin"], "cpu": ["arm64"]},
                      "package/bin/magistr": native})
        write_json(self.root / "cargo-inventory.json", cargo)
        (self.root / "publication-order.txt").write_text("magistr-cli\n")
        write_json(self.root / "npm-inventory.json",
                   {"name": "magistr", "version": version, "filename": npm.name})
        write_json(self.root / "binary-inventory.json",
                   {"version": version, "target": "aarch64-apple-darwin",
                    "binarySha256": hashlib.sha256(native).hexdigest()})
        (self.root / "SHA256SUMS").write_text(
            f"{release.digest(crate)}  {crate.name}\n{release.digest(npm)}  {npm.name}\n")
        write_json(self.root / "cargo-install-verification.json",
                   {"passed": True, "version": version, "profile": "release", "checkoutRequired": False,
                    "artifacts": {crate.name: release.digest(crate)}})
        write_json(self.root / "npm-install-verification.json",
                   {"passed": True, "version": version, "artifactSha256": release.digest(npm),
                    "rustOrBunOnPath": False, "globalInstall": True, "npxInit": True,
                    "pinnedConfigFreshCache": True, "nativeMcpAndWorker": True, "sigterm": True,
                    "removal": True, "usageExitCode": 2})

        generated = datetime.now(timezone.utc).isoformat()
        write_json(self.root / "security-verification.json", {
            "schema": 1, "passed": True, "source": SOURCE, "sourceDirty": False,
            "target": "aarch64-apple-darwin", "generatedAt": generated,
            "artifactSha256": {crate.name: release.digest(crate), npm.name: release.digest(npm)},
            "inputSha256": {"Cargo.lock": "a" * 64},
            "advisoryDatabase": {"last-commit": "b" * 40, "fetchedAt": generated},
            "checks": {name: {"passed": True}
                       for name in ("cargo", "npm:apps/website", "npm:scripts", "secrets")},
        })

    def test_missing_failed_stale_or_unbound_security_receipts_are_refused(self):
        path = self.root / "security-verification.json"
        original = release.read_json(path)
        for changes in ({"passed": False}, {"source": "2" * 40}, {"artifactSha256": {}},
                        {"generatedAt": "2026-01-01T00:00:00+00:00"}, {"sourceDirty": True}):
            with self.subTest(changes=changes):
                write_json(path, dict(original, **changes))
                with self.assertRaises(ValueError):
                    release.verify(self.root, SOURCE)
        path.unlink()
        with self.assertRaises(FileNotFoundError):
            release.verify(self.root, SOURCE)

    def test_reviewed_archives_and_matching_receipts_are_accepted(self):
        result = release.verify(self.root, SOURCE)
        self.assertEqual(result["version"], "0.1.0")
        self.assertEqual(result["npmTag"], "latest")

    def test_pre_release_artifacts_publish_on_the_beta_tag(self):
        for path in self.root.iterdir():
            path.unlink()
        self.build("1.0.0-beta.1")
        result = release.verify(self.root, SOURCE)
        self.assertEqual(result["version"], "1.0.0-beta.1")
        self.assertEqual(result["npm"], "magistr-1.0.0-beta.1.tgz")
        self.assertEqual(result["npmTag"], "beta")

    def test_invalid_archive_version_is_refused_before_reading_paths(self):
        path = self.root / "cargo-inventory.json"
        records = release.read_json(path)
        records[0]["version"] = "../outside"
        write_json(path, records)
        with self.assertRaisesRegex(ValueError, "Invalid release version"):
            release.verify(self.root, SOURCE)

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
