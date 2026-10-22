#!/usr/bin/env python3
"""Scan the release source and bind current security results to its artifacts."""
import argparse
from datetime import date, datetime, timedelta, timezone
import hashlib
import io
import json
from pathlib import Path
import platform
import re
import subprocess
import tarfile
import tempfile
import tomllib
import urllib.request

ROOT = Path(__file__).resolve().parent.parent
POLICY = ROOT / "scripts/release-security-policy.json"
TARGET = "aarch64-apple-darwin"
MAX_RECEIPT_AGE = timedelta(hours=24)
CARGO_AUDIT_VERSION = "0.22.1"
GITLEAKS_VERSION = "8.30.1"
GITLEAKS_RELEASE = "https://github.com/gitleaks/gitleaks/releases/download/v8.30.1/"
# Reviewed scanner binaries per CI host. The dependency graph is always evaluated
# for the macOS release target, so the scan itself may run on either host.
GITLEAKS_PINS = {
    ("Darwin", "arm64"): ("gitleaks_8.30.1_darwin_arm64.tar.gz",
                          "b40ab0ae55c505963e365f271a8d3846efbc170aa17f2607f13df610a9aeb6a5"),
    ("Linux", "x86_64"): ("gitleaks_8.30.1_linux_x64.tar.gz",
                          "551f6fc83ea457d62a0d98237cbad105af8d557003051f41f3e7ca7b3f2470eb"),
}
REQUIRED_CHECKS = {"cargo", "npm:apps/website", "npm:scripts", "secrets"}


def require(condition, message):
    if not condition:
        raise ValueError(message)


def host_pin(system=None, machine=None):
    """The reviewed gitleaks archive for this host; other hosts are refused."""
    host = (system or platform.system(), machine or platform.machine())
    require(host in GITLEAKS_PINS,
            "Release security scans run on macOS arm64 or Linux x86_64 hosts; "
            f"{host[0]} {host[1]} has no reviewed scanner pin")
    archive, digest = GITLEAKS_PINS[host]
    return GITLEAKS_RELEASE + archive, digest


def sha256(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def timestamp(value):
    result = datetime.fromisoformat(value.replace("Z", "+00:00"))
    require(result.tzinfo is not None, "Security timestamp has no timezone")
    return result


def invoke(args, cwd=ROOT, accepted=(0,)):
    result = subprocess.run(args, cwd=cwd, text=True, capture_output=True, timeout=600)
    require(result.returncode in accepted,
            f"{Path(args[0]).name} failed with exit {result.returncode}; security scan incomplete")
    return result.stdout


def git(*args):
    return invoke(["git", *args]).strip()


def enabled_packages(tree):
    packages = set()
    for line in tree.splitlines():
        match = re.match(r"^([A-Za-z0-9_-]+) v([^ ]+)(?: |$)", line)
        require(match is not None, "Unexpected Cargo dependency graph output")
        packages.add(match.groups())
    require(any(name == "magistr-cli" for name, _ in packages), "CLI graph is empty")
    return packages


def reviewed_exception(record, exceptions, today, root=ROOT):
    matches = [item for item in exceptions
               if (item.get("advisory"), item.get("package"), item.get("version"), item.get("target"))
               == (record["advisory"], record["package"], record["version"], TARGET)]
    require(len(matches) <= 1, "Duplicate dependency security exception")
    if not matches:
        return None
    item = matches[0]
    reviewed = date.fromisoformat(item["reviewedOn"])
    expires = date.fromisoformat(item["expiresOn"])
    require(reviewed <= today <= expires and expires - reviewed <= timedelta(days=90),
            f"Expired or invalid dependency review: {record['advisory']}")
    for field in ("reason", "featurePath", "upstream"):
        require(isinstance(item.get(field), str) and item[field].strip(),
                f"Dependency exception requires {field}")
    require(item.get("evidence"), "Dependency exception requires source evidence")
    for evidence in item["evidence"]:
        path = Path(evidence["file"])
        require(not path.is_absolute() and ".." not in path.parts,
                "Dependency exception evidence escapes source")
        require(sha256(root / path) == evidence["sha256"],
                f"Dependency exception evidence changed: {path}")
    return item


def cargo_findings(report, enabled, exceptions, today, root=ROOT):
    settings = report["settings"]
    require(settings.get("ignore") == [] and settings.get("severity") is None,
            "Cargo audit may not suppress advisories")
    require(set(settings.get("informational_warnings", [])) >= {"unmaintained", "unsound", "notice"},
            "Cargo audit must include maintenance and soundness warnings")
    records = []
    findings = [("vulnerability", item) for item in report["vulnerabilities"]["list"]]
    for kind, items in report.get("warnings", {}).items():
        findings.extend((kind, item) for item in items)
    for kind, item in findings:
        package = item["package"]
        advisory = item.get("advisory") or {}
        record = {"kind": kind, "package": package["name"], "version": package["version"],
                  "advisory": advisory.get("id", f"{kind}:{package['name']}"),
                  "title": advisory.get("title", kind),
                  "patched": item.get("versions", {}).get("patched", []),
                  "status": "blocked"}
        if (record["package"], record["version"]) not in enabled:
            record["status"] = "outside-enabled-cli-graph"
        else:
            review = reviewed_exception(record, exceptions, today, root)
            if review is not None:
                record["status"] = "reviewed-exception"
                record["review"] = review
        records.append(record)
    return {"passed": not any(item["status"] == "blocked" for item in records),
            "enabledPackages": len(enabled), "findings": records}


def secret_findings(findings, reviews):
    reviewed = {}
    for item in reviews:
        key = (item["commit"], item["file"], item["rule"], item["line"])
        require(key not in reviewed and item["contextVerified"] is True
                and bool(item["reason"]), "Invalid or duplicate secret review")
        require(re.fullmatch(r"[0-9a-f]{40}", item["commit"]) is not None,
                "Secret reviews require an immutable commit")
        reviewed[key] = item["reason"]
    records = []
    for item in findings:
        key = (item["Commit"], item["File"], item["RuleID"], item["StartLine"])
        reason = reviewed.get(key)
        records.append({"commit": key[0], "file": key[1], "rule": key[2], "line": key[3],
                        "status": "reviewed-fixture" if reason else "blocked",
                        "reason": reason})
    return {"passed": all(item["status"] != "blocked" for item in records),
            "findings": records, "scope": "complete source ancestry",
            "inlineAllowComments": False}


NPM_AUDIT_ATTEMPTS = 3
NPM_AUDIT_TIMEOUT_MS = 60000


def is_advisory_timeout(log):
    return "audit network timeout" in log


def npm_audit(project, run=subprocess.run, attempts=NPM_AUDIT_ATTEMPTS):
    """Audit one npm project; a hung advisory request is retried a bounded number of times."""
    log = ""
    for attempt in range(1, attempts + 1):
        audit = run(["npm", "audit", "--json", "--ignore-scripts",
                     "--registry", "https://registry.npmjs.org",
                     f"--fetch-timeout={NPM_AUDIT_TIMEOUT_MS}", "--fetch-retries=0"],
                    cwd=ROOT / project, text=True, capture_output=True, timeout=600)
        log += f"[attempt {attempt}]\n{audit.stderr.strip()}\n"
        require(audit.returncode in (0, 1),
                f"npm audit for {project} failed with exit {audit.returncode}; security scan incomplete"
                f"\nnpm log:\n{log.strip()[-1500:]}")
        if not is_advisory_timeout(audit.stderr) or attempt == attempts:
            return json.loads(audit.stdout), log
    raise AssertionError("unreachable")


def npm_findings(report, project="npm", log=""):
    error = report.get("error") if isinstance(report, dict) else None
    detail = f": {json.dumps(error, sort_keys=True)[:1500]}" if error is not None else ""
    log = log.strip()[-1500:]
    if is_advisory_timeout(log):
        detail = ("; npm's advisory service did not answer (see https://status.npmjs.org), "
                  "rerun the scan once it does" + detail)
    require(error is None and report.get("auditReportVersion") == 2,
            f"npm audit for {project} did not return a complete report{detail}"
            + (f"\nnpm log:\n{log}" if log else ""))
    records = [{"package": name, "severity": item["severity"],
                "advisories": [{"title": via["title"], "url": via["url"],
                               "range": via.get("range")}
                              for via in item["via"] if isinstance(via, dict)]}
               for name, item in report["vulnerabilities"].items()]
    return {"passed": not records, "findings": records}


def artifact_hashes(directory):
    if directory is None:
        return {}
    records = {}
    for line in (directory / "SHA256SUMS").read_text().splitlines():
        checksum, name = line.split()
        require(re.fullmatch(r"[0-9a-f]{64}", checksum) and Path(name).name == name
                and name not in records, "Invalid or duplicate artifact checksum")
        require(not (directory / name).is_symlink() and sha256(directory / name) == checksum,
                f"Artifact checksum differs: {name}")
        records[name] = checksum
    require(records, "Artifact inventory is empty")
    return records


def verify_receipt(path, source, artifact_hashes, now=None):
    receipt = json.loads(path.read_text())
    require(receipt.get("schema") == 1 and receipt.get("passed") is True,
            "Release security checks have not passed")
    require(receipt.get("source") == source and receipt.get("sourceDirty") is False,
            "Security receipt does not describe the clean release source")
    require(receipt.get("target") == TARGET, "Security receipt has another target")
    current = now or datetime.now(timezone.utc)
    age = current - timestamp(receipt["generatedAt"])
    require(timedelta(minutes=-5) <= age <= MAX_RECEIPT_AGE,
            "Security receipt is stale or from the future; run current scans again")
    require(bool(artifact_hashes) and receipt.get("artifactSha256") == artifact_hashes,
            "Security receipt belongs to different artifacts")
    checks = receipt.get("checks", {})
    require(set(checks) == REQUIRED_CHECKS and all(item.get("passed") is True for item in checks.values()),
            "Security receipt is missing a required successful scanner")
    database = receipt.get("advisoryDatabase", {})
    require(re.fullmatch(r"[0-9a-f]{40}", database.get("last-commit", "")) is not None,
            "Security receipt has no advisory database revision")
    fetched = timestamp(database["fetchedAt"])
    require(timedelta(minutes=-5) <= current - fetched <= MAX_RECEIPT_AGE,
            "Security advisory database was not refreshed recently")
    require(bool(receipt.get("inputSha256")), "Security receipt has no input identity")
    return {"passed": True, "source": source, "generatedAt": receipt["generatedAt"],
            "target": TARGET, "artifactCount": len(artifact_hashes)}


def check_release_configuration(root=ROOT, workflows=None):
    rust = tomllib.loads((root / "rust-toolchain.toml").read_text())["toolchain"]["channel"]
    for name, version in (("Rust", rust), ("Node.js", (root / ".node-version").read_text().strip()),
                          ("Python", (root / ".python-version").read_text().strip())):
        require(re.fullmatch(r"[0-9]+\.[0-9]+\.[0-9]+", version) is not None,
                f"{name} release toolchain must use a complete version")
    pending = list(workflows) if workflows is not None else [
        path.relative_to(root).as_posix()
        for pattern in ("*.yml", "*.yaml") for path in (root / ".github/workflows").glob(pattern)]
    require(pending, "No workflows found for security verification")
    checked = set()
    while pending:
        name = pending.pop()
        if name in checked:
            continue
        checked.add(name)
        for line in (root / name).read_text().splitlines():
            tool = re.match(r"^\s*tool:\s*(.*?)\s*(?:#.*)?$", line)
            if tool is not None:
                value = tool[1].strip("'\"")
                # The scanner action reads its pinned version from this script.
                generated = value == "${{ steps.tools.outputs.cargo-audit }}" and name == ".github/actions/release-security/action.yml"
                require(generated or re.fullmatch(r"[A-Za-z0-9_-]+@[0-9]+\.[0-9]+\.[0-9]+", value),
                        f"Installed release tool must use a complete version: {value}")
            match = re.match(r"^\s*(?:-\s*)?uses:\s*([^\s#]+)", line)
            if match is None:
                continue
            action = match[1].strip("'\"")
            if action.startswith("./"):
                relative = Path(action)
                require(".." not in relative.parts, "Local release action escapes source")
                candidates = ([relative] if relative.suffix in (".yml", ".yaml")
                              else [relative / "action.yml", relative / "action.yaml"])
                candidate = next((item for item in candidates if (root / item).is_file()), None)
                require(candidate is not None, f"Local release action is missing: {action}")
                pending.append(candidate.as_posix())
            else:
                require(re.fullmatch(r"[A-Za-z0-9_.-]+/[A-Za-z0-9_./-]+@[0-9a-f]{40}", action),
                        f"Release action must use a full commit SHA: {action}")
    return checked


def source_inputs():
    # Enumerate Git inputs, without walking generated build/package trees.
    tracked = git("ls-files").splitlines()
    paths = {ROOT / name for name in tracked if Path(name).name == "Cargo.toml"}
    paths.update(ROOT / name for name in check_release_configuration())
    for name in ("Cargo.lock", "rust-toolchain.toml", ".node-version", ".python-version", "scripts/package-lock.json",
                 "scripts/package.json", "apps/website/package-lock.json", "apps/website/package.json",
                 "scripts/release-security.py", "scripts/release-security-policy.json"):
        paths.add(ROOT / name)
    return {path.relative_to(ROOT).as_posix(): sha256(path) for path in sorted(paths)}


def scan(source, output, artifacts=None, allow_dirty=False, gitleaks="gitleaks"):
    # Failed or interrupted rescans must never leave an earlier green receipt in place.
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_text(json.dumps({"schema": 1, "passed": False, "source": source,
                                  "checks": {}, "failure": "Scan incomplete"}) + "\n")
    require(re.fullmatch(r"[0-9a-f]{40}", source) is not None and git("rev-parse", "HEAD") == source,
            "Security scan must use the checked-out source revision")
    host_pin()  # the scanner pin exists for this host
    require(git("rev-parse", "--is-shallow-repository") == "false",
            "Secret scan requires full Git history (checkout fetch-depth: 0)")
    dirty = bool(git("status", "--porcelain", "--untracked-files=normal"))
    require(allow_dirty or not dirty, "Security scan requires a clean source tree")
    inputs = source_inputs()
    artifacts_before = artifact_hashes(artifacts)
    policy = json.loads(POLICY.read_text())
    require(policy["schema"] == 1, "Unsupported security review policy")
    started = datetime.now(timezone.utc)
    require(invoke(["cargo", "audit", "--version"]).split()[-1] == CARGO_AUDIT_VERSION,
            "Install the pinned cargo-audit version")
    require(invoke([gitleaks, "version"]).strip() == GITLEAKS_VERSION,
            "Install the pinned gitleaks version")
    tree = invoke(["cargo", "tree", "--locked", "-p", "magistr-cli", "--target", TARGET,
                   "--edges", "normal,build", "--prefix", "none", "--format", "{p}"])
    with tempfile.TemporaryDirectory(prefix="magistr-release-security-") as temp:
        temporary = Path(temp)
        # An explicit config prevents repository or environment allowlists from hiding findings.
        config = temporary / "gitleaks.toml"
        config.write_text("[extend]\nuseDefault = true\n")
        ignore = temporary / "empty-ignore"
        ignore.write_text("")
        leaks = temporary / "secrets.json"
        invoke([gitleaks, "git", str(ROOT), "--log-opts", f"--full-history {source}",
                "--redact=100", "--no-banner", "--no-color", "--ignore-gitleaks-allow",
                "--config", str(config), "--gitleaks-ignore-path", str(ignore),
                "--report-format", "json", "--report-path", str(leaks)], accepted=(0, 1))
        secrets = secret_findings(json.loads(leaks.read_text()), policy["secretReviews"])
        # No --no-fetch, --stale, --ignore or cached audit result: refresh RustSec now.
        cargo = json.loads(invoke(["cargo", "audit", "--json", "--file", str(ROOT / "Cargo.lock"),
                                  "--target-arch", "aarch64", "--target-os", "macos"],
                                 cwd=temporary, accepted=(0, 1)))
    checks = {"cargo": cargo_findings(cargo, enabled_packages(tree),
                                     policy["dependencyExceptions"], started.date()),
              "secrets": secrets}
    for project in ("apps/website", "scripts"):
        report, log = npm_audit(project)
        checks[f"npm:{project}"] = npm_findings(report, project, log)
    require(source_inputs() == inputs and git("rev-parse", "HEAD") == source,
            "Security inputs changed during the scan; run again")
    require(artifact_hashes(artifacts) == artifacts_before, "Artifacts changed during the security scan")
    dirty = dirty or bool(git("status", "--porcelain", "--untracked-files=normal"))
    passed = all(item["passed"] for item in checks.values())
    receipt = {"schema": 1, "source": source, "sourceDirty": dirty, "target": TARGET,
               "generatedAt": datetime.now(timezone.utc).isoformat(), "passed": passed,
               "artifactSha256": artifacts_before, "inputSha256": inputs,
               "tools": {"cargoAudit": CARGO_AUDIT_VERSION, "gitleaks": GITLEAKS_VERSION,
                         "npm": invoke(["npm", "--version"]).strip()},
               "advisoryDatabase": dict(cargo["database"], fetchedAt=started.isoformat()),
               "checks": checks}
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_text(json.dumps(receipt, indent=2) + "\n")
    print(json.dumps({"passed": passed, "sourceDirty": dirty, "receipt": str(output),
                      "checks": {name: {"passed": item["passed"], "findings": len(item["findings"])}
                                 for name, item in checks.items()}}))
    return passed


def install_gitleaks(directory):
    url, digest = host_pin()
    with urllib.request.urlopen(url, timeout=60) as response:
        archive = response.read()
    require(hashlib.sha256(archive).hexdigest() == digest,
            "Gitleaks download checksum differs")
    with tarfile.open(fileobj=io.BytesIO(archive), mode="r:gz") as package:
        member = package.getmember("gitleaks")
        require(member.isfile(), "Gitleaks archive has no regular executable")
        binary = package.extractfile(member).read()
    directory.mkdir(parents=True, exist_ok=True)
    executable = directory / "gitleaks"
    executable.write_bytes(binary)
    executable.chmod(0o755)
    print(directory.resolve())


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="action", required=True)
    check = commands.add_parser("scan")
    check.add_argument("--source", required=True)
    check.add_argument("--out", type=Path, required=True)
    check.add_argument("--artifacts", type=Path)
    check.add_argument("--allow-dirty", action="store_true",
                       help="Local diagnostics only; receipt cannot authorize publication")
    check.add_argument("--gitleaks", default="gitleaks",
                       help="Path of the pinned gitleaks executable (default: resolved on PATH)")
    install = commands.add_parser("install-gitleaks")
    install.add_argument("--out", type=Path, required=True)
    commands.add_parser("tools")
    commands.add_parser("configuration")
    args = parser.parse_args()
    try:
        if args.action == "scan":
            return 0 if scan(args.source, args.out, args.artifacts, args.allow_dirty, args.gitleaks) else 1
        if args.action == "configuration":
            print(json.dumps({"passed": True, "files": sorted(check_release_configuration())}))
            return 0
        if args.action == "install-gitleaks":
            install_gitleaks(args.out)
        else:
            print(f"cargo-audit=cargo-audit@{CARGO_AUDIT_VERSION}")
        return 0
    except (ValueError, KeyError, OSError, subprocess.TimeoutExpired, tarfile.TarError) as error:
        raise SystemExit(f"Release security refused: {error}") from error


if __name__ == "__main__":
    raise SystemExit(main())
