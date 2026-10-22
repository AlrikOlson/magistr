#!/usr/bin/env python3
"""Verify reviewed CLI artifacts; publish only through the explicit publish-cargo command."""
import argparse
import hashlib
import importlib.util
import os
import json
from pathlib import Path
import re
import subprocess
import tarfile
from cargo_publication import Publisher
from version import SEMVER, npm_dist_tag


def security_verifier():
    spec = importlib.util.spec_from_file_location("release_security", Path(__file__).with_name("release-security.py"))
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module.verify_receipt


def require(condition, message):
    if not condition:
        raise ValueError(message)


def read_json(path):
    return json.loads(path.read_text())


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def check_run(run, repository):
    require(run["repository"]["full_name"] == repository, "Package run belongs to another repository")
    require(run["path"] == ".github/workflows/cli-packages.yml", "Run is not CLI packages")
    require(run["event"] == "workflow_dispatch", "Package run was not manually selected")
    require(run["head_branch"] == "main", "Package run must use main")
    require(run["status"] == "completed" and run["conclusion"] == "success", "Package run did not pass")
    sha = run["head_sha"]
    require(re.fullmatch(r"[0-9a-f]{40}", sha) is not None, "Invalid package source revision")
    return sha


def check_quality(runs, sha, label="release-readiness"):
    matching = [run for run in runs if run["head_sha"] == sha and run["event"] == "push"]
    require(matching, f"No {label} CI run exists for the package source")
    latest = max(matching, key=lambda run: run["run_number"])
    require(latest["status"] == "completed" and latest["conclusion"] == "success",
            f"Latest {label} CI for the package source has not passed")


def github(path):
    return json.loads(subprocess.check_output(["gh", "api", path], text=True))


def select_run(repository, run_id, resume_run=None):
    require(re.fullmatch(r"[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+", repository) is not None,
            "Invalid repository")
    require(run_id.isdecimal(), "Package run ID must be numeric")
    run = github(f"repos/{repository}/actions/runs/{run_id}")
    sha = check_run(run, repository)
    checks = github(f"repos/{repository}/actions/workflows/public-release.yml/runs?head_sha={sha}&per_page=100")
    check_quality(checks["workflow_runs"], sha)
    security = github(f"repos/{repository}/actions/workflows/release-security.yml/runs?head_sha={sha}&per_page=100")
    check_quality(security["workflow_runs"], sha, "release-security")
    if resume_run:
        require(resume_run.isdecimal(), "Previous publication run ID must be numeric")
        previous = github(f"repos/{repository}/actions/runs/{resume_run}")
        require(previous["repository"]["full_name"] == repository
                and previous["path"] == ".github/workflows/cli-publish.yml"
                and previous["event"] == "workflow_dispatch" and previous["head_branch"] == "main"
                and previous["status"] == "completed", "Previous publication run is not a completed manual release")
    print(sha)


def verify(artifacts, source):
    require(re.fullmatch(r"[0-9a-f]{40}", source) is not None, "Invalid source revision")
    cargo = read_json(artifacts / "cargo-inventory.json")
    require(bool(cargo), "Cargo inventory is empty")
    versions = {record["version"] for record in cargo}
    require(len(versions) == 1, "Cargo package versions differ")
    version = versions.pop()
    require(isinstance(version, str) and SEMVER.fullmatch(version), "Invalid release version")
    names = [record["package"] for record in cargo]
    require(len(set(names)) == len(names) and names[-1] == "magistr-cli",
            "Cargo publication order is incomplete or duplicated")
    require((artifacts / "publication-order.txt").read_text().splitlines() == names,
            "Cargo publication order differs from the inventory")
    expected = {}
    for record in cargo:
        name = record["package"]
        require(re.fullmatch(r"magistr-[a-z0-9-]+", name) is not None, "Unexpected Cargo package name")
        require(record["file"] == f"{name}-{version}.crate", "Cargo archive name differs")
        path = artifacts / record["file"]
        require(digest(path) == record["sha256"], f"Cargo archive checksum differs: {name}")
        with tarfile.open(path) as archive:
            vcs = json.load(archive.extractfile(f"{name}-{version}/.cargo_vcs_info.json"))
        require(vcs["git"]["sha1"] == source and not vcs["git"].get("dirty", False),
                f"Cargo archive does not describe the clean release revision: {name}")
        expected[record["file"]] = record["sha256"]

    npm = read_json(artifacts / "npm-inventory.json")
    require(npm["name"] == "magistr" and npm["version"] == version, "npm/Cargo package identities differ")
    require(npm["filename"] == f"magistr-{version}.tgz", "Unexpected npm archive name")
    path = artifacts / npm["filename"]
    expected[path.name] = digest(path)
    binary = read_json(artifacts / "binary-inventory.json")
    require(binary["version"] == version and binary["target"] == "aarch64-apple-darwin",
            "Binary inventory has another version or target")
    with tarfile.open(path) as archive:
        manifest = json.load(archive.extractfile("package/package.json"))
        require(manifest["name"] == "magistr" and manifest["version"] == version,
                "Packed npm manifest differs")
        require(manifest["os"] == ["darwin"] and manifest["cpu"] == ["arm64"],
                "Packed npm platform differs")
        packed_binary = archive.extractfile("package/bin/magistr").read()
        require(hashlib.sha256(packed_binary).hexdigest() == binary["binarySha256"],
                "Packed npm binary differs from its inventory")
    sums = {}
    for line in (artifacts / "SHA256SUMS").read_text().splitlines():
        checksum, name = line.split()
        require(name not in sums, "Duplicate artifact checksum")
        sums[name] = checksum
    require(sums == expected, "Artifact checksums differ from the inventories")

    cargo_check = read_json(artifacts / "cargo-install-verification.json")
    require(cargo_check["passed"] is True and cargo_check["version"] == version
            and cargo_check["profile"] == "release" and cargo_check["checkoutRequired"] is False,
            "Extracted release-profile Cargo installation was not verified")
    require(cargo_check["artifacts"] == {record["file"]: record["sha256"] for record in cargo},
            "Cargo installation receipt belongs to different artifacts")
    npm_check = read_json(artifacts / "npm-install-verification.json")
    require(npm_check["passed"] is True and npm_check["version"] == version
            and npm_check["artifactSha256"] == expected[path.name]
            and npm_check["rustOrBunOnPath"] is False,
            "npm installation receipt belongs to different artifacts")
    for field in ("globalInstall", "npxInit", "pinnedConfigFreshCache", "nativeMcpAndWorker",
                  "sigterm", "removal"):
        require(npm_check[field] is True, f"npm installation did not verify {field}")
    require(npm_check["usageExitCode"] == 2, "npm usage exit status was not verified")
    security_verifier()(artifacts / "security-verification.json", source, expected)
    # The npm dist-tag follows the reviewed artifact version, not the checkout:
    # a pre-release publishes on `beta`, a stable version on `latest`.
    return {"version": version, "source": source, "cargoPackages": len(cargo),
            "npm": npm["filename"], "npmTag": npm_dist_tag(version), "verified": True}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="action", required=True)
    select = commands.add_parser("select")
    select.add_argument("--repository", required=True)
    select.add_argument("--run-id", required=True)
    select.add_argument("--resume-run", help="Completed publication run containing the previous receipt")
    check = commands.add_parser("verify")
    check.add_argument("--out", type=Path, required=True)
    check.add_argument("--source", required=True)
    publish = commands.add_parser("publish-cargo", help="Upload exact reviewed archives to crates.io")
    publish.add_argument("--out", type=Path, required=True)
    publish.add_argument("--source", required=True)
    publish.add_argument("--max-wait", type=int, default=7200, help="Total wait budget in seconds")
    args = parser.parse_args()
    try:
        if args.action == "select":
            select_run(args.repository, args.run_id, args.resume_run)
        else:
            result = verify(args.out, args.source)
            if args.action == "publish-cargo":
                result = Publisher(args.out, args.source, os.environ.get("CARGO_REGISTRY_TOKEN"),
                                   max_wait=args.max_wait).run()
            print(json.dumps(result))
    except (ValueError, KeyError, OSError, tarfile.TarError) as error:
        raise SystemExit(f"Release verification refused: {error}") from error


if __name__ == "__main__":
    main()
