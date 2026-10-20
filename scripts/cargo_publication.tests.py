#!/usr/bin/env python3
"""Publication protocol and failure recovery checks. No public registry writes."""
from email.message import Message
from email.utils import formatdate
from http.client import IncompleteRead
import hashlib
import io
import json
from pathlib import Path
import struct
import tarfile
import tempfile
import unittest
from unittest.mock import patch
import urllib.error
import urllib.request

import cargo_publication as publication

SOURCE = "1" * 40


def make_archive(root, name, dependency=None, extra=""):
    prefix = f"{name}-0.1.0"
    manifest = f'[package]\nname = "{name}"\nversion = "0.1.0"\ndescription = "Fixture"\nlicense = "MIT"\nreadme = "README.md"\n'
    if dependency:
        manifest += f'\n[dependencies.{dependency}]\nversion = "=0.1.0"\n'
    manifest += extra
    path = root / f"{prefix}.crate"
    with tarfile.open(path, "w:gz") as archive:
        for relative, text in (("Cargo.toml", manifest), ("README.md", "Fixture README\n")):
            data = text.encode()
            info = tarfile.TarInfo(f"{prefix}/{relative}")
            info.size = len(data)
            archive.addfile(info, io.BytesIO(data))
    return {"package": name, "version": "0.1.0", "file": path.name,
            "sha256": hashlib.sha256(path.read_bytes()).hexdigest()}


class Clock:
    def __init__(self):
        self.elapsed, self.sleeps = 0, []

    def monotonic(self):
        return self.elapsed

    def wall_time(self):
        return 1_800_000_000 + self.elapsed

    def sleep(self, seconds):
        self.sleeps.append(seconds)
        self.elapsed += seconds


class Registry:
    def __init__(self, root, records):
        self.root, self.records = root, {r["package"]: r for r in records}
        self.published, self.outcomes, self.index_delays = {}, {}, {}
        self.requests, self.puts, self.get_failures = [], [], []
        self.corrupt_download = False

    def request(self, method, url, body=None, token=None):
        self.requests.append((method, url))
        if method == "PUT":
            size = struct.unpack("<I", body[:4])[0]
            metadata = json.loads(body[4:4 + size])
            payload_size = struct.unpack("<I", body[4 + size:8 + size])[0]
            payload = body[8 + size:]
            assert len(payload) == payload_size
            name = metadata["name"]
            assert payload == (self.root / self.records[name]["file"]).read_bytes()
            assert token == "fixture"
            self.puts.append(name)
            outcomes = self.outcomes.setdefault(name, [])
            outcome = outcomes.pop(0) if outcomes else "success"
            if isinstance(outcome, tuple):
                return outcome
            if outcome == "timeout-before-acceptance":
                raise TimeoutError()
            self.published[name] = dict(self.records[name])
            if outcome == "lost-response":
                raise TimeoutError()
            if outcome == "truncated-response":
                raise IncompleteRead(b"partial")
            if outcome == "server-error-after-acceptance":
                return 503, {}, b"unavailable"
            if outcome == "invalid-success-response":
                return 200, {}, b"not json"
            return 200, {}, b'{"warnings": {"other": []}}'
        assert token is None
        if self.get_failures:
            failure = self.get_failures.pop(0)
            if failure is not None:
                return failure
        if url.startswith(publication.API + "/api/v1/crates/"):
            name = url.split("/")[-2]
            record = self.published.get(name)
            if not record:
                return 404, {}, b""
            return 200, {}, json.dumps({"version": {
                "crate": name, "num": "0.1.0", "checksum": record["sha256"],
                "yanked": record.get("yanked", False)}}).encode()
        if url.startswith(publication.INDEX + "/"):
            name = url.split("/")[-1]
            record = self.published.get(name)
            if not record:
                return 404, {}, b""
            delay = self.index_delays.get(name, 0)
            if delay:
                self.index_delays[name] -= 1
                return 404, {}, b""
            return 200, {}, json.dumps({
                "name": name, "vers": "0.1.0", "cksum": record.get("index_checksum", record["sha256"]),
                "yanked": record.get("yanked", False)}).encode()
        if url.startswith(publication.DOWNLOAD + "/"):
            payload = (self.root / url.split("/")[-1]).read_bytes()
            return 200, {}, b"corrupt" if self.corrupt_download else payload
        raise AssertionError(url)


class PublicationTests(unittest.TestCase):
    def setUp(self):
        self.directory = tempfile.TemporaryDirectory(prefix="publication fixtures ")
        self.addCleanup(self.directory.cleanup)
        self.root = Path(self.directory.name)
        self.records = [make_archive(self.root, "magistr-domain"),
                        make_archive(self.root, "magistr-cli", "magistr-domain")]
        self.write_inventory()
        self.clock = Clock()
        self.registry = Registry(self.root, self.records)

    def write_inventory(self):
        (self.root / "cargo-inventory.json").write_text(json.dumps(self.records))
        (self.root / "publication-order.txt").write_text(
            "".join(record["package"] + "\n" for record in self.records))

    def publisher(self, **kwargs):
        return publication.Publisher(self.root, SOURCE, "fixture", transport=self.registry,
                                     monotonic=self.clock.monotonic, sleep=self.clock.sleep,
                                     wall_time=self.clock.wall_time, index_wait=12, **kwargs)

    def receipt(self):
        return json.loads((self.root / "cargo-publication.json").read_text())

    def test_exact_archives_are_uploaded_in_order_and_downloads_verified(self):
        result = self.publisher().run()
        self.assertTrue(result["complete"])
        self.assertEqual(self.registry.puts, ["magistr-domain", "magistr-cli"])
        self.assertTrue(all(p["registrySha256"] == p["sha256"] for p in result["packages"]))

    def test_existing_exact_versions_resume_without_upload(self):
        self.registry.published = {r["package"]: dict(r) for r in self.records}
        result = self.publisher().run()
        self.assertEqual(self.registry.puts, [])
        self.assertTrue(all(p["effect"] == "already-published" for p in result["packages"]))

    def test_conflict_in_later_package_is_found_before_first_effect(self):
        self.registry.published["magistr-cli"] = dict(self.records[1], sha256="0" * 64)
        with self.assertRaisesRegex(ValueError, "Published checksum differs"):
            self.publisher().run()
        self.assertEqual(self.registry.puts, [])

    def test_yanked_version_stops_without_implicitly_unyanking(self):
        self.registry.published["magistr-domain"] = dict(self.records[0], yanked=True)
        with self.assertRaisesRegex(ValueError, "yanked"):
            self.publisher().run()
        self.assertEqual(self.registry.puts, [])

    def test_index_checksum_conflict_stops(self):
        self.registry.published["magistr-domain"] = dict(self.records[0], index_checksum="0" * 64)
        with self.assertRaisesRegex(ValueError, "checksum differs in index"):
            self.publisher().run()
        self.assertEqual(self.registry.puts, [])

    def test_mismatched_registry_archive_stops_later_uploads(self):
        self.registry.corrupt_download = True
        with self.assertRaisesRegex(ValueError, "Published archive differs"):
            self.publisher().run()
        self.assertEqual(self.registry.puts, ["magistr-domain"])

    def test_index_delay_does_not_repeat_successful_upload(self):
        self.registry.index_delays["magistr-domain"] = 2
        self.publisher().run()
        self.assertEqual(self.registry.puts, ["magistr-domain", "magistr-cli"])
        self.assertEqual(self.clock.elapsed, 10)

    def test_lost_response_is_reconciled_without_replay(self):
        self.registry.outcomes["magistr-domain"] = ["lost-response"]
        result = self.publisher().run()
        self.assertEqual(result["packages"][0]["effect"], "reconciled")
        self.assertEqual(self.registry.puts, ["magistr-domain", "magistr-cli"])

    def test_truncated_response_is_reconciled_without_replay(self):
        self.registry.outcomes["magistr-domain"] = ["truncated-response"]
        result = self.publisher().run()
        self.assertEqual(result["packages"][0]["effect"], "reconciled")
        self.assertEqual(self.registry.puts, ["magistr-domain", "magistr-cli"])

    def test_server_error_after_acceptance_is_reconciled_without_replay(self):
        self.registry.outcomes["magistr-domain"] = ["server-error-after-acceptance"]
        self.publisher().run()
        self.assertEqual(self.registry.puts, ["magistr-domain", "magistr-cli"])

    def test_malformed_success_is_reconciled_without_replay(self):
        self.registry.outcomes["magistr-domain"] = ["invalid-success-response"]
        self.publisher().run()
        self.assertEqual(self.registry.puts, ["magistr-domain", "magistr-cli"])

    def test_ambiguous_failure_stops_with_completed_effects_and_no_automatic_replay(self):
        self.registry.outcomes["magistr-cli"] = ["timeout-before-acceptance"]
        with self.assertRaisesRegex(ValueError, "wait limit"):
            self.publisher().run()
        result = self.receipt()
        self.assertFalse(result["complete"])
        self.assertEqual([p["state"] for p in result["packages"]], ["verified", "upload-uncertain"])
        self.assertEqual(self.registry.puts, ["magistr-domain", "magistr-cli"])
        self.registry.puts.clear()
        with self.assertRaisesRegex(ValueError, "wait limit"):
            self.publisher().run()
        self.assertEqual(self.registry.puts, [])  # Retained uncertainty never authorizes a retry.

    def test_resumption_confirms_completed_ambiguous_effect_and_continues(self):
        self.registry.outcomes["magistr-domain"] = ["timeout-before-acceptance"]
        with self.assertRaises(ValueError):
            self.publisher().run()
        self.registry.published["magistr-domain"] = dict(self.records[0])
        self.registry.puts.clear()
        result = self.publisher().run()
        self.assertTrue(result["complete"])
        self.assertEqual(self.registry.puts, ["magistr-cli"])

    def test_explicit_rate_limit_honors_retry_after_then_reconciles_before_retry(self):
        self.registry.outcomes["magistr-domain"] = [(429, {"retry-after": "7"}, b"")]
        self.publisher().run()
        self.assertEqual(self.clock.elapsed, 7)
        self.assertEqual(self.registry.puts, ["magistr-domain", "magistr-domain", "magistr-cli"])
        first = next(i for i, row in enumerate(self.registry.requests) if row[0] == "PUT")
        self.assertEqual(self.registry.requests[first + 1][0], "GET")

    def test_rate_limit_exceeding_budget_stops_instead_of_early_retry(self):
        self.registry.outcomes["magistr-domain"] = [(429, {"retry-after": "600"}, b"")]
        with self.assertRaisesRegex(ValueError, "wait limit"):
            self.publisher(max_wait=30).run()
        self.assertEqual(self.registry.puts, ["magistr-domain"])
        self.assertEqual(self.clock.elapsed, 0)
        self.assertEqual(self.receipt()["packages"][0]["state"], "rate-limited")

    def test_resume_honors_rate_limit_deadline_from_previous_run(self):
        self.registry.outcomes["magistr-domain"] = [(429, {"retry-after": "600"}, b"")]
        with self.assertRaises(ValueError):
            self.publisher(max_wait=30).run()
        self.clock.sleep(100)
        self.publisher().run()
        self.assertEqual(self.clock.elapsed, 600)

    def test_failed_preflight_preserves_prior_uncertain_effects(self):
        self.registry.outcomes["magistr-cli"] = ["timeout-before-acceptance"]
        with self.assertRaises(ValueError):
            self.publisher().run()
        self.registry.get_failures = [(503, {}, b"")] * 10
        with self.assertRaises(ValueError):
            self.publisher(max_wait=6).run()
        self.assertEqual(self.receipt()["packages"][1]["state"], "upload-uncertain")
        self.registry.get_failures.clear()
        self.registry.puts.clear()
        with self.assertRaises(ValueError):
            self.publisher().run()
        self.assertEqual(self.registry.puts, [])

    def test_authentication_rejection_stops_immediately(self):
        self.registry.outcomes["magistr-domain"] = [(403, {}, b"")]
        with self.assertRaisesRegex(ValueError, "HTTP 403"):
            self.publisher().run()
        self.assertEqual(self.registry.puts, ["magistr-domain"])

    def test_read_failures_are_bounded_without_uploads(self):
        self.registry.get_failures = [(503, {}, b"")] * 10
        with self.assertRaisesRegex(ValueError, "timed out"):
            self.publisher(max_wait=6).run()
        self.assertEqual(self.registry.puts, [])

    def test_read_rate_limit_honors_header(self):
        self.registry.get_failures = [(429, {"retry-after": "9"}, b"")]
        self.publisher().run()
        self.assertEqual(self.clock.elapsed, 9)

    def test_interruption_is_persisted_before_upload(self):
        original = self.registry.request
        def interrupted(method, *args, **kwargs):
            if method == "PUT":
                raise KeyboardInterrupt()
            return original(method, *args, **kwargs)
        self.registry.request = interrupted
        with self.assertRaises(KeyboardInterrupt):
            self.publisher().run()
        self.assertEqual(self.receipt()["packages"][0]["state"], "upload-uncertain")
        self.assertIn("stoppedAt", self.receipt())

    def test_previous_receipt_from_other_artifacts_is_refused(self):
        self.publisher().run()
        receipt = self.receipt()
        receipt["source"] = "2" * 40
        (self.root / "cargo-publication.json").write_text(json.dumps(receipt))
        with self.assertRaisesRegex(ValueError, "different reviewed artifacts"):
            self.publisher()

    def test_reordered_dependency_graph_and_changed_archive_are_refused(self):
        self.records.reverse()
        self.write_inventory()
        with self.assertRaises(ValueError):
            self.publisher()
        self.records.reverse()
        self.write_inventory()
        (self.root / self.records[0]["file"]).write_bytes(b"changed")
        with self.assertRaisesRegex(ValueError, "changed before"):
            self.publisher()


class MetadataTests(unittest.TestCase):
    def test_normalized_metadata_retains_readme_and_dependency_semantics(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            record = make_archive(root, "magistr-cli", extra="""
[features]
default = ["optional"]
optional = ["dep:serde_alias"]
[dependencies.serde_alias]
package = "serde"
version = "1"
optional = true
default-features = false
features = ["derive"]
[build-dependencies]
cc = "1"
[target.'cfg(unix)'.dev-dependencies]
libc = "0.2"
""")
            value = publication.archive_metadata(root / record["file"], record)
            self.assertEqual(value["readme"], "Fixture README\n")
            self.assertEqual(value["readme_file"], "README.md")
            self.assertEqual(value["deps"][0], {
                "name": "serde", "version_req": "1", "features": ["derive"], "optional": True,
                "default_features": False, "target": None, "kind": "normal",
                "registry": None, "explicit_name_in_toml": "serde_alias"})
            self.assertEqual(value["deps"][1]["kind"], "build")
            self.assertEqual(value["deps"][2]["target"], "cfg(unix)")
            self.assertEqual(value["deps"][2]["kind"], "dev")

    def test_workspace_or_path_dependencies_are_not_silently_published(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            record = make_archive(root, "magistr-cli", extra='\n[dependencies.fake]\npath = "../fake"\nversion = "1"\n')
            with self.assertRaisesRegex(ValueError, "not normalized"):
                publication.archive_metadata(root / record["file"], record)

    def test_retry_after_accepts_dates_and_bounds_bad_inputs(self):
        self.assertEqual(publication.retry_after("12"), 12)
        self.assertEqual(publication.retry_after(formatdate(110, usegmt=True), lambda: 100), 10)
        for value in (None, "bad", "-10", "inf"):
            self.assertEqual(publication.retry_after(value), 600)
        self.assertEqual(publication.retry_after("0"), 1)


class TransportTests(unittest.TestCase):
    def test_token_cannot_be_sent_to_another_host_or_read(self):
        client = publication.Transport()
        for method, url in (("PUT", "https://example.com"), ("GET", publication.API)):
            with self.assertRaisesRegex(ValueError, "Credentials"):
                client.request(method, url, token="fixture")

    def test_redirects_never_forward_an_upload_or_token(self):
        request = urllib.request.Request(publication.API + "/api/v1/crates/new",
                                         data=b"fixture", method="PUT",
                                         headers={"Authorization": "fixture"})
        result = publication.NoRedirect().redirect_request(
            request, None, 307, "redirect", {}, "https://example.com")
        self.assertIsNone(result)

    def test_retry_header_is_normalized_and_http_error_retains_status(self):
        client = publication.Transport()
        headers = Message()
        headers["Retry-After"] = "600"
        error = urllib.error.HTTPError(publication.API, 429, "limited", headers, io.BytesIO(b"limited"))
        with patch.object(client.opener, "open", side_effect=error):
            status, actual, body = client.request("GET", publication.API)
        self.assertEqual((status, actual["retry-after"], body), (429, "600", b"limited"))


if __name__ == "__main__":
    unittest.main()
