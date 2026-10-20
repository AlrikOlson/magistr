#!/usr/bin/env python3
"""Publish reviewed Cargo archives, reconciling every effect against crates.io.

The registry owns publication state; receipts describe observed effects, never
authorize skipping a checksum check. Only an explicit HTTP 429 permits retrying
an upload. Lost responses and server errors trigger read-only reconciliation.
"""
from datetime import datetime, timezone
from email.utils import parsedate_to_datetime
import hashlib
from http.client import HTTPException
import json
import math
from pathlib import PurePosixPath
import struct
import tarfile
import time
import tomllib
import urllib.error
import urllib.request

API = "https://crates.io"
INDEX = "https://index.crates.io"
DOWNLOAD = "https://static.crates.io/crates"
USER_AGENT = "magistr-release (https://github.com/AlrikOlson/magistr)"


def require(condition, message):
    if not condition:
        raise ValueError(message)


def archive_metadata(path, record):
    """Translate the normalized manifest into Cargo's documented publish metadata."""
    root = f"{record['package']}-{record['version']}"
    with tarfile.open(path) as archive:
        def read(relative):
            name = PurePosixPath(relative)
            require(not name.is_absolute() and ".." not in name.parts,
                    "Archive metadata refers outside its package")
            member = archive.getmember(f"{root}/{name}")
            require(member.isfile(), f"Archive metadata is not a file: {relative}")
            return archive.extractfile(member).read().decode("utf-8")

        manifest = tomllib.loads(read("Cargo.toml"))
        package = manifest["package"]
        require(package["name"] == record["package"] and package["version"] == record["version"],
                "Cargo manifest differs from its reviewed identity")
        require(package.get("publish", True) is not False, "Cargo manifest prohibits publication")
        require(package.get("publish", ["crates-io"]) in (True, ["crates-io"]),
                "Cargo manifest selects another registry")
        readme_file = package.get("readme")
        require(readme_file is None or readme_file is False or isinstance(readme_file, str),
                "Cargo README must be normalized before publication")
        result = {key: package.get(key) for key in
                  ("description", "documentation", "homepage", "license", "repository", "links")}
        result.update(name=package["name"], vers=package["version"],
                      authors=package.get("authors", []), keywords=package.get("keywords", []),
                      categories=package.get("categories", []),
                      license_file=package.get("license-file"),
                      rust_version=package.get("rust-version"),
                      readme=read(readme_file) if readme_file else None,
                      readme_file=readme_file or None,
                      features=manifest.get("features", {}), badges=manifest.get("badges", {}),
                      deps=[])

    tables = [(None, manifest), *sorted(manifest.get("target", {}).items())]
    for target, table in tables:
        for key, kind in (("dependencies", "normal"), ("build-dependencies", "build"),
                          ("dev-dependencies", "dev")):
            for alias, dependency in sorted(table.get(key, {}).items()):
                if isinstance(dependency, str):
                    dependency = {"version": dependency}
                require(not any(key in dependency for key in ("path", "git", "workspace", "registry")),
                        f"Dependency is not normalized for crates.io: {alias}")
                require(isinstance(dependency.get("version"), str),
                        f"Dependency has no registry version: {alias}")
                result["deps"].append({
                    "name": dependency.get("package", alias),
                    "version_req": dependency["version"],
                    "features": dependency.get("features", []),
                    "optional": dependency.get("optional", False),
                    "default_features": dependency.get("default-features", True),
                    "target": target, "kind": kind,
                    "registry": dependency.get("registry-index"),
                    "explicit_name_in_toml": alias if "package" in dependency else None,
                })
    return result


def upload_body(metadata, archive_bytes):
    encoded = json.dumps(metadata, separators=(",", ":")).encode("utf-8")
    require(max(len(encoded), len(archive_bytes)) < 2 ** 32, "Cargo upload exceeds protocol size")
    return struct.pack("<I", len(encoded)) + encoded + struct.pack("<I", len(archive_bytes)) + archive_bytes


def index_path(name):
    name = name.lower()
    if len(name) < 3:
        return f"{len(name)}/{name}"
    if len(name) == 3:
        return f"3/{name[0]}/{name}"
    return f"{name[:2]}/{name[2:4]}/{name}"


def retry_after(value, wall_time=time.time):
    """Honor both standard Retry-After formats; never convert invalid input to a tight loop."""
    if value:
        try:
            delay = float(value) if value.isdecimal() else parsedate_to_datetime(value).timestamp() - wall_time()
            if math.isfinite(delay):
                return max(1.0, delay)
        except (ValueError, TypeError, OverflowError):
            pass
    return 600.0  # crates.io's documented new-crate refill interval; still bounded by the deadline.


class NoRedirect(urllib.request.HTTPRedirectHandler):
    def redirect_request(self, req, fp, code, msg, headers, newurl):
        # Never forward a registry token or an upload to an unreviewed endpoint.
        return None


class Transport:
    def __init__(self):
        self.opener = urllib.request.build_opener(NoRedirect())

    def request(self, method, url, body=None, token=None):
        headers = {"User-Agent": USER_AGENT, "Accept": "application/json", "Cache-Control": "no-cache"}
        if token is not None:
            require(method == "PUT" and url == f"{API}/api/v1/crates/new",
                    "Credentials may only be sent to the crates.io publish endpoint")
            headers.update(Authorization=token, **{"Content-Type": "application/octet-stream"})
        request = urllib.request.Request(url, data=body, headers=headers, method=method)
        try:
            with self.opener.open(request, timeout=30) as response:
                return response.status, {key.lower(): value for key, value in response.headers.items()}, response.read()
        except urllib.error.HTTPError as error:
            with error:
                return error.code, {key.lower(): value for key, value in error.headers.items()}, error.read()


class Publisher:
    def __init__(self, artifacts, source, token, *, max_wait=7200, index_wait=300,
                 transport=None, monotonic=time.monotonic, sleep=time.sleep, wall_time=time.time):
        require(max_wait > 0 and index_wait > 0, "Publication time limits must be positive")
        require(bool(token), "Set CARGO_REGISTRY_TOKEN before publishing")
        self.artifacts, self.token = artifacts, token
        self.transport = transport or Transport()
        self.monotonic, self.sleep, self.wall_time = monotonic, sleep, wall_time
        self.deadline, self.index_wait = monotonic() + max_wait, index_wait
        self.records = json.loads((artifacts / "cargo-inventory.json").read_text())
        self.metadata = {}
        names = [record["package"] for record in self.records]
        require(len(names) == len(set(names)) and names and names[-1] == "magistr-cli",
                "Invalid Cargo publication inventory")
        require((artifacts / "publication-order.txt").read_text().splitlines() == names,
                "Cargo publication order differs")
        seen = set()
        for record in self.records:
            path = artifacts / record["file"]
            require(record["file"] == f"{record['package']}-{record['version']}.crate",
                    "Cargo archive filename differs")
            require(hashlib.sha256(path.read_bytes()).hexdigest() == record["sha256"],
                    f"Cargo archive changed before publication: {record['package']}")
            metadata = archive_metadata(path, record)
            for dependency in metadata["deps"]:
                if dependency["name"].startswith("magistr-"):
                    require(dependency["name"] in names and dependency["version_req"] == f"={record['version']}",
                            f"Unreviewed internal dependency: {dependency['name']}")
                    require(dependency["name"] in seen,
                            f"Cargo dependency must publish first: {dependency['name']}")
            self.metadata[record["package"]] = metadata
            seen.add(record["package"])
        previous_path = artifacts / "cargo-publication.json"
        previous = json.loads(previous_path.read_text()) if previous_path.exists() else None
        hashes = {r["file"]: r["sha256"] for r in self.records}
        if previous:
            require(previous["schema"] == 1 and previous["source"] == source
                    and previous["registry"] == API and previous["artifacts"] == hashes,
                    "Previous publication receipt belongs to different reviewed artifacts")
        self.previous = {p["package"]: p for p in previous["packages"]} if previous else {}
        self.receipt = {
            "schema": 1, "source": source, "registry": API, "complete": False,
            "artifacts": hashes,
            "packages": [dict(self.previous.get(r["package"], {})) or {
                "package": r["package"], "version": r["version"], "sha256": r["sha256"],
                "state": "unattempted", "uploads": 0} for r in self.records],
            "startedAt": self.timestamp(),
        }

    def timestamp(self):
        return datetime.fromtimestamp(self.wall_time(), timezone.utc).isoformat()

    def save(self):
        path = self.artifacts / "cargo-publication.json"
        temporary = path.with_suffix(".json.tmp")
        temporary.write_text(json.dumps(self.receipt, indent=2) + "\n")
        temporary.replace(path)

    def pause(self, seconds, deadline, reason):
        remaining = min(deadline, self.deadline) - self.monotonic()
        require(remaining > 0 and seconds <= remaining,
                f"Publication wait limit reached ({reason}); inspect cargo-publication.json and registry state")
        print(f"{reason}; waiting {math.ceil(seconds)} seconds", flush=True)
        # Short sleeps keep cancellation and progress reporting responsive.
        while seconds > 0:
            part = min(seconds, 30)
            self.sleep(part)
            seconds -= part

    def get(self, url, deadline=None):
        deadline = min(deadline or self.deadline, self.deadline)
        delay = 2
        while True:
            require(self.monotonic() < deadline, "Registry read timed out; no upload was retried")
            try:
                status, headers, body = self.transport.request("GET", url)
            except (OSError, TimeoutError, HTTPException):
                status, headers, body = 503, {}, b""
            if status == 429:
                self.pause(retry_after(headers.get("retry-after"), self.wall_time), deadline,
                           "Registry read rate limited")
            elif status >= 500:
                self.pause(delay, deadline, "Registry read unavailable")
                delay = min(delay * 2, 30)
            else:
                require(status in (200, 404), f"Registry read refused with HTTP {status}")
                return status, body

    def observe(self, record, deadline=None):
        name, version = record["package"], record["version"]
        status, body = self.get(f"{API}/api/v1/crates/{name}/{version}", deadline)
        published = None
        if status == 200:
            published = json.loads(body)["version"]
            require(published["crate"] == name and published["num"] == version,
                    f"Registry returned another package: {name}")
            require(published["checksum"] == record["sha256"],
                    f"Published checksum differs: {name}@{version}; stop and review the immutable version")
            require(published["yanked"] is False, f"Published version is yanked: {name}@{version}")
        status, body = self.get(f"{INDEX}/{index_path(name)}", deadline)
        indexed = []
        if status == 200:
            indexed = [row for line in body.splitlines() if line
                       for row in [json.loads(line)] if row["vers"] == version]
        require(len(indexed) <= 1, f"Registry index duplicates {name}@{version}")
        if indexed:
            row = indexed[0]
            require(row["name"] == name and row["cksum"] == record["sha256"],
                    f"Published checksum differs in index: {name}@{version}")
            require(row["yanked"] is False, f"Indexed version is yanked: {name}@{version}")
        return bool(published), bool(indexed)

    def confirm(self, record, entry, deadline, *, allow_absent=False):
        while True:
            published, indexed = self.observe(record, deadline)
            if published and indexed:
                url = f"{DOWNLOAD}/{record['package']}/{record['file']}"
                status, body = self.get(url, deadline)
                if status == 200:
                    require(hashlib.sha256(body).hexdigest() == record["sha256"],
                            f"Published archive differs: {record['package']}")
                    entry.update(state="verified", registrySha256=record["sha256"], verifiedAt=self.timestamp())
                    self.save()
                    return True
            if allow_absent and not published and not indexed:
                return False
            self.pause(5, deadline, f"Waiting for registry confirmation of {record['package']}")

    def publish_one(self, record):
        entry = next(p for p in self.receipt["packages"] if p["package"] == record["package"])
        previous = self.previous.get(record["package"], {})
        uncertain = previous.get("state") in ("upload-uncertain", "awaiting-index", "verified")
        entry["state"] = "upload-uncertain" if uncertain else "checking"
        self.save()
        if self.confirm(record, entry, min(self.deadline, self.monotonic() + self.index_wait),
                        allow_absent=not uncertain):
            entry["effect"] = "already-published"
            self.save()
            return
        delayed_until = previous.get("retryAfterUntil", 0)
        if delayed_until > self.wall_time():
            self.pause(delayed_until - self.wall_time(), self.deadline,
                       f"Honoring previous rate limit for {record['package']}")
        data = (self.artifacts / record["file"]).read_bytes()
        require(hashlib.sha256(data).hexdigest() == record["sha256"], "Cargo archive changed during publication")
        body = upload_body(self.metadata[record["package"]], data)
        while True:
            require(self.monotonic() < self.deadline, "Publication time limit reached before upload")
            entry.update(state="upload-uncertain", uploads=entry["uploads"] + 1)
            self.save()  # Persist uncertainty before the effect, including interruption mid-request.
            try:
                status, headers, response = self.transport.request(
                    "PUT", f"{API}/api/v1/crates/new", body, self.token)
            except (OSError, TimeoutError, HTTPException):
                status, headers, response = 0, {}, b""
            if status == 429:
                delay = retry_after(headers.get("retry-after"), self.wall_time)
                entry.update(state="rate-limited", lastHttpStatus=429,
                             retryAfterUntil=self.wall_time() + delay)
                self.save()
                self.pause(delay, self.deadline,
                           f"Cargo upload rate limited for {record['package']}")
                # Even a definitely rejected upload is reconciled before another PUT.
                if self.confirm(record, entry, min(self.deadline, self.monotonic() + self.index_wait),
                                allow_absent=True):
                    entry["effect"] = "reconciled"
                    self.save()
                    return
                continue
            accepted = 200 <= status < 300
            if accepted:
                try:
                    result = json.loads(response)
                    accepted = isinstance(result, dict) and not result.get("errors")
                    if accepted:
                        entry["warnings"] = result.get("warnings", {})
                except (ValueError, TypeError):
                    accepted = False
            entry.update(state="awaiting-index" if accepted else "upload-uncertain",
                         lastHttpStatus=status or None)
            self.save()
            # No retry after ambiguous outcomes. Only GETs can resolve this request.
            deadline = min(self.deadline, self.monotonic() + self.index_wait)
            if not accepted and 400 <= status < 500:
                if not self.confirm(record, entry, deadline, allow_absent=True):
                    entry["state"] = "rejected"
                    self.save()
                    raise ValueError(f"Cargo upload refused with HTTP {status} for {record['package']}; "
                                     "no further upload was attempted")
            else:
                self.confirm(record, entry, deadline)
            entry["effect"] = "uploaded" if accepted else "reconciled"
            self.save()
            return

    def run(self):
        self.save()
        try:
            # Discover known conflicts anywhere in the batch before its first mutation.
            for record in self.records:
                self.observe(record)
            for record in self.records:
                self.publish_one(record)
            self.receipt.update(complete=True, completedAt=self.timestamp())
            self.save()
            return self.receipt
        except BaseException:
            self.receipt["stoppedAt"] = self.timestamp()
            self.save()
            raise
