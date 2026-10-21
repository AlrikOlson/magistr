#!/usr/bin/env python3
"""Security gate regression checks: target selection, reviewed scope and stale evidence."""
from datetime import datetime, timedelta, timezone
import importlib.util
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

spec = importlib.util.spec_from_file_location("release_security", Path(__file__).with_name("release-security.py"))
security = importlib.util.module_from_spec(spec)
spec.loader.exec_module(security)
SOURCE = "1" * 40
NOW = datetime(2026, 10, 20, tzinfo=timezone.utc)


class SecurityGateTests(unittest.TestCase):
    def setUp(self):
        self.directory = tempfile.TemporaryDirectory()
        self.addCleanup(self.directory.cleanup)
        self.root = Path(self.directory.name)

    def report(self, package="example", kind="vulnerability"):
        finding = {"advisory": {"id": "RUSTSEC-2026-0001", "title": "Regression fixture"},
                   "package": {"name": package, "version": "1.2.3"}, "versions": {"patched": [">=1.2.4"]}}
        return {"settings": {"ignore": [], "severity": None,
                             "informational_warnings": ["unmaintained", "unsound", "notice"]},
                "vulnerabilities": {"list": [finding] if kind == "vulnerability" else []},
                "warnings": {} if kind == "vulnerability" else {kind: [finding]}}

    def test_disabled_optional_dependency_is_recorded_and_enabled_finding_blocks(self):
        enabled = security.enabled_packages("magistr-cli v0.1.0 (/project)\nexample v1.2.3\nexample v1.2.3 (*)")
        result = security.cargo_findings(self.report(), enabled, [], NOW.date())
        self.assertFalse(result["passed"])
        self.assertEqual(result["findings"][0]["status"], "blocked")
        result = security.cargo_findings(self.report("optional-transport"), enabled, [], NOW.date())
        self.assertTrue(result["passed"])
        self.assertEqual(result["findings"][0]["status"], "outside-enabled-cli-graph")

    def test_unsound_and_unmaintained_enabled_crates_block(self):
        for kind in ("unsound", "unmaintained", "notice"):
            with self.subTest(kind=kind):
                self.assertFalse(security.cargo_findings(
                    self.report(kind=kind), {("example", "1.2.3")}, [], NOW.date())["passed"])

    def test_partial_or_suppressed_audit_configuration_is_refused(self):
        for settings in ({"ignore": ["RUSTSEC-2026-0001"]}, {"severity": "high"},
                         {"informational_warnings": ["unmaintained"]}):
            report = self.report()
            report["settings"].update(settings)
            with self.subTest(settings=settings), self.assertRaises(ValueError):
                security.cargo_findings(report, {("example", "1.2.3")}, [], NOW.date())

    def exception(self):
        evidence = self.root / "verification.rs"
        evidence.write_text("verification uses only a public key\n")
        return {"advisory": "RUSTSEC-2026-0001", "package": "example", "version": "1.2.3",
                "target": security.TARGET, "reviewedOn": "2026-10-19", "expiresOn": "2026-11-19",
                "reason": "The vulnerable private-key operation is not used.",
                "featurePath": "CLI > authentication > public verification",
                "upstream": "https://rustsec.org/advisories/RUSTSEC-2026-0001.html",
                "evidence": [{"file": evidence.name, "sha256": security.sha256(evidence)}]}

    def test_reviewed_exception_requires_current_unchanged_source_and_exact_version(self):
        exception = self.exception()
        accepted = security.cargo_findings(self.report(), {("example", "1.2.3")},
                                          [exception], NOW.date(), self.root)
        self.assertTrue(accepted["passed"])
        changed = dict(exception, version="1.2.2")
        self.assertFalse(security.cargo_findings(self.report(), {("example", "1.2.3")},
                                                [changed], NOW.date(), self.root)["passed"])
        (self.root / "verification.rs").write_text("also decrypts private-key messages\n")
        with self.assertRaisesRegex(ValueError, "evidence changed"):
            security.cargo_findings(self.report(), {("example", "1.2.3")},
                                    [exception], NOW.date(), self.root)

    def test_expired_overlong_and_unexplained_exceptions_are_refused(self):
        original = self.exception()
        for changes in ({"expiresOn": "2026-10-18"}, {"expiresOn": "2027-10-19"},
                        {"featurePath": ""}, {"reason": ""}, {"evidence": []}):
            with self.subTest(changes=changes), self.assertRaises(ValueError):
                security.cargo_findings(self.report(), {("example", "1.2.3")},
                                        [dict(original, **changes)], NOW.date(), self.root)

    def test_secret_review_cannot_suppress_another_commit_line_file_or_rule(self):
        finding = {"Commit": SOURCE, "File": "tests/redaction.rs", "StartLine": 9, "RuleID": "jwt",
                   "Secret": "must never be copied into the receipt"}
        review = {"commit": SOURCE, "file": finding["File"], "line": 9, "rule": "jwt",
                  "contextVerified": True, "reason": "Deliberate fixture"}
        result = security.secret_findings([finding], [review])
        self.assertTrue(result["passed"])
        self.assertNotIn("Secret", result["findings"][0])
        for changes in ({"Commit": "2" * 40}, {"File": "src/auth.rs"}, {"StartLine": 10},
                        {"RuleID": "generic-api-key"}):
            with self.subTest(changes=changes):
                self.assertFalse(security.secret_findings([dict(finding, **changes)], [review])["passed"])

    def test_unreviewed_or_duplicate_secret_allowances_are_refused(self):
        review = {"commit": SOURCE, "file": "fixture", "line": 1, "rule": "jwt",
                  "contextVerified": False, "reason": "Not checked"}
        with self.assertRaises(ValueError):
            security.secret_findings([], [review])
        review["contextVerified"] = True
        with self.assertRaises(ValueError):
            security.secret_findings([], [review, review])

    def test_npm_findings_at_any_severity_block_and_registry_errors_are_not_clean(self):
        self.assertTrue(security.npm_findings({"auditReportVersion": 2, "vulnerabilities": {}})["passed"])
        self.assertFalse(security.npm_findings({"auditReportVersion": 2, "vulnerabilities": {
            "example": {"severity": "low", "via": ["transitive"]}}})["passed"])
        with self.assertRaises(ValueError):
            security.npm_findings({"error": {"code": "network"}})

    def receipt(self):
        return {"schema": 1, "passed": True, "source": SOURCE, "sourceDirty": False,
                "target": security.TARGET, "generatedAt": NOW.isoformat(),
                "artifactSha256": {"magistr.tgz": "a" * 64}, "inputSha256": {"Cargo.lock": "b" * 64},
                "advisoryDatabase": {"last-commit": "c" * 40, "fetchedAt": NOW.isoformat()},
                "checks": {name: {"passed": True} for name in security.REQUIRED_CHECKS}}

    def verify(self, receipt):
        path = self.root / "security-verification.json"
        path.write_text(json.dumps(receipt))
        return security.verify_receipt(path, SOURCE, {"magistr.tgz": "a" * 64}, NOW)

    def test_current_source_and_artifact_bound_security_receipt_is_accepted(self):
        self.assertTrue(self.verify(self.receipt())["passed"])

    def test_security_receipts_fail_closed_on_wrong_identity_or_missing_scanner(self):
        for changes in ({"passed": False}, {"sourceDirty": True}, {"source": "2" * 40},
                        {"target": "x86_64-pc-windows-msvc"}, {"artifactSha256": {}},
                        {"checks": {}}, {"advisoryDatabase": {}}, {"inputSha256": {}}):
            with self.subTest(changes=changes), self.assertRaises((ValueError, KeyError)):
                self.verify(dict(self.receipt(), **changes))
        receipt = self.receipt()
        receipt["checks"]["secrets"]["passed"] = False
        with self.assertRaises(ValueError):
            self.verify(receipt)

    def test_stale_future_and_naive_timestamps_are_refused(self):
        for value in (NOW - timedelta(hours=25), NOW + timedelta(hours=1), NOW.replace(tzinfo=None)):
            receipt = self.receipt()
            receipt["generatedAt"] = value.isoformat()
            with self.subTest(value=value), self.assertRaises(ValueError):
                self.verify(receipt)
        receipt = self.receipt()
        receipt["advisoryDatabase"]["fetchedAt"] = (NOW - timedelta(hours=25)).isoformat()
        with self.assertRaises(ValueError):
            self.verify(receipt)

    def test_failed_rescan_invalidates_previous_green_receipt(self):
        path = self.root / "security-verification.json"
        path.write_text(json.dumps(self.receipt()))
        with patch.object(security, "git", return_value="2" * 40), self.assertRaises(ValueError):
            security.scan(SOURCE, path)
        self.assertFalse(json.loads(path.read_text())["passed"])

    def test_artifact_inventory_refuses_replaced_bytes_and_path_escape(self):
        package = self.root / "magistr.tgz"
        package.write_bytes(b"reviewed artifact")
        sums = self.root / "SHA256SUMS"
        sums.write_text(f"{security.sha256(package)}  magistr.tgz\n")
        self.assertEqual(len(security.artifact_hashes(self.root)), 1)
        package.write_bytes(b"different artifact")
        with self.assertRaises(ValueError):
            security.artifact_hashes(self.root)
        sums.write_text(f"{'a' * 64}  ../elsewhere.tgz\n")
        with self.assertRaises(ValueError):
            security.artifact_hashes(self.root)

    def configuration_fixture(self):
        (self.root / "rust-toolchain.toml").write_text('[toolchain]\nchannel = "1.99.1"\n')
        (self.root / ".node-version").write_text("24.22.0\n")
        (self.root / ".python-version").write_text("3.12.15\n")
        (self.root / "workflow.yml").write_text("steps:\n  - uses: ./nested\n")
        (self.root / "nested").mkdir()
        (self.root / "nested/action.yml").write_text("steps:\n  - uses: owner/action@" + "a" * 40 + "\n")

    def test_release_configuration_checks_nested_local_actions(self):
        self.configuration_fixture()
        self.assertEqual(security.check_release_configuration(self.root, ["workflow.yml"]),
                         {"workflow.yml", "nested/action.yml"})
        (self.root / "nested/action.yml").write_text("steps:\n  - uses: owner/action@v4\n")
        with self.assertRaisesRegex(ValueError, "full commit SHA"):
            security.check_release_configuration(self.root, ["workflow.yml"])

    def test_release_configuration_follows_reusable_workflows(self):
        self.configuration_fixture()
        (self.root / "workflow.yml").write_text("jobs:\n  check:\n    uses: ./nested/desktop.yml\n")
        (self.root / "nested/desktop.yml").write_text("steps:\n  - uses: owner/action@v4\n")
        with self.assertRaisesRegex(ValueError, "full commit SHA"):
            security.check_release_configuration(self.root, ["workflow.yml"])

    def test_release_configuration_refuses_floating_toolchains_and_missing_local_actions(self):
        self.configuration_fixture()
        (self.root / ".node-version").write_text("24\n")
        with self.assertRaisesRegex(ValueError, "complete version"):
            security.check_release_configuration(self.root, ["workflow.yml"])
        (self.root / ".node-version").write_text("24.22.0\n")
        (self.root / "workflow.yml").write_text("steps:\n  - uses: ./missing\n")
        with self.assertRaisesRegex(ValueError, "missing"):
            security.check_release_configuration(self.root, ["workflow.yml"])

    def test_release_configuration_refuses_floating_installed_tools(self):
        self.configuration_fixture()
        action = self.root / "nested/action.yml"
        prefix = "steps:\n  - uses: owner/action@" + "a" * 40 + "\n    with:\n      tool: "
        for value in ("cargo-nextest", "cargo-nextest@latest", "cargo-nextest@0.9",
                      "${{ inputs.version }}"):
            action.write_text(prefix + value + "\n")
            with self.subTest(value=value), self.assertRaisesRegex(ValueError, "complete version"):
                security.check_release_configuration(self.root, ["workflow.yml"])
        action.write_text(prefix + "cargo-nextest@0.9.130\n")
        security.check_release_configuration(self.root, ["workflow.yml"])

    def test_empty_or_unrecognized_dependency_graph_does_not_silently_pass(self):
        for graph in ("", "magistr-cli v0.1.0\nunparsed row", "dependency v1.0.0"):
            with self.subTest(graph=graph), self.assertRaises(ValueError):
                security.enabled_packages(graph)


if __name__ == "__main__":
    unittest.main()
