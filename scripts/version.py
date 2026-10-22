#!/usr/bin/env python3
"""Own Magistr's release version. Cargo.toml [workspace.package] is authoritative."""
import argparse
import json
from pathlib import Path
import re
import subprocess
import tomllib

ROOT = Path(__file__).resolve().parents[1]
NUMBER = r"(?:0|[1-9][0-9]*)"
IDENTIFIER = rf"(?:{NUMBER}|[0-9]*[A-Za-z-][0-9A-Za-z-]*)"
SEMVER = re.compile(rf"{NUMBER}\.{NUMBER}\.{NUMBER}(?P<prerelease>-{IDENTIFIER}(?:\.{IDENTIFIER})*)?(?:\+[0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*)?")
JSON_COPIES = ("npm/magistr/package.json", "plugins/magistr/.claude-plugin/plugin.json")
PRIVATE_PACKAGES = ("apps/website/package.json", "crates/magistr-desktop/package.json")
DESKTOP = "crates/magistr-desktop/Cargo.toml"
# Release channels. A semver pre-release such as 1.0.0-beta.1 is published and
# verified on the npm `beta` dist-tag; `latest` is reserved for stable versions.
# Cargo has no dist-tags: `cargo install` skips pre-releases unless the exact
# version is named, so pre-release install instructions must pin it.
PRERELEASE_TAG = "beta"
STABLE_TAG = "latest"


def toml(path):
    return tomllib.loads(path.read_text())


def product_version(root=ROOT):
    version = toml(root / "Cargo.toml")["workspace"]["package"]["version"]
    if not isinstance(version, str) or not SEMVER.fullmatch(version):
        raise ValueError("workspace.package.version must be a semantic version")
    return version


def npm_dist_tag(version):
    """The npm dist-tag a product version is published and verified on."""
    match = SEMVER.fullmatch(version) if isinstance(version, str) else None
    if match is None:
        raise ValueError("Use a semantic version such as 1.0.0 or 1.0.0-beta.1")
    return PRERELEASE_TAG if match.group("prerelease") else STABLE_TAG


def members(root):
    result = {}
    for pattern in toml(root / "Cargo.toml")["workspace"]["members"]:
        paths = sorted(root.glob(pattern + "/Cargo.toml"))
        if not paths:
            raise ValueError(f"Workspace member has no manifest: {pattern}")
        for path in paths:
            manifest = toml(path)
            name = manifest["package"]["name"]
            if name in result:
                raise ValueError(f"Duplicate workspace package: {name}")
            result[name] = path
    return result


def replace_field(source, section, key, value):
    """Replace one string field in one TOML section without reformatting it."""
    sections = re.split(r"(?m)(?=^\[[^\n]+\]\s*$)", source)
    matches = 0
    for index, block in enumerate(sections):
        if block.splitlines()[:1] != [f"[{section}]"]:
            continue
        block, count = re.subn(
            rf'(?m)^({re.escape(key)}\s*=\s*)"[^"\n]*"',
            lambda match: match[1] + json.dumps(value), block)
        sections[index] = block
        matches += count
    if matches != 1:
        raise ValueError(f"Expected one {section}.{key} string")
    return "".join(sections)


def dependency_tables(manifest):
    for kind in ("dependencies", "dev-dependencies", "build-dependencies"):
        yield manifest.get(kind, {})
    for target in manifest.get("target", {}).values():
        yield from dependency_tables(target)


def generated_changes(root, version):
    if not SEMVER.fullmatch(version):
        raise ValueError("Use a semantic version such as 0.2.0 or 0.2.0-rc.1")
    packages = members(root)
    workspace = toml(root / "Cargo.toml")["workspace"]
    source = (root / "Cargo.toml").read_text()
    source = replace_field(source, "workspace.package", "version", version)
    for name, path in packages.items():
        manifest = toml(path)
        if manifest["package"].get("version") != {"workspace": True}:
            raise ValueError(f"{path.relative_to(root)} must inherit version.workspace")
        for dependencies in dependency_tables(manifest):
            for alias, dependency in dependencies.items():
                package = dependency.get("package", alias) if isinstance(dependency, dict) else alias
                if package in packages and (
                    not isinstance(dependency, dict) or dependency.get("workspace") is not True
                    or "version" in dependency or "path" in dependency
                ):
                    raise ValueError(f"{path.relative_to(root)}: {alias} must use workspace inheritance")
        dependency = workspace["dependencies"].get(name, {})
        if dependency.get("path") != str(path.parent.relative_to(root)):
            raise ValueError(f"workspace.dependencies.{name} must point to its member")
        pattern = rf'(?m)^({re.escape(name)}\s*=\s*\{{\s*version\s*=\s*)"[^"\n]*"'
        source, count = re.subn(pattern, lambda match: match[1] + json.dumps("=" + version), source)
        if count != 1:
            raise ValueError(f"Expected one generated registry requirement for {name}")

    generated = {root / "Cargo.toml": source}
    desktop = root / DESKTOP
    generated[desktop] = replace_field(desktop.read_text(), "package", "version", version)
    for relative in JSON_COPIES:
        path = root / relative
        data = json.loads(path.read_text())
        data["version"] = version
        generated[path] = json.dumps(data, indent=2, ensure_ascii=False) + "\n"

    # Build-only npm packages and Tauri use their existing owners, not extra versions.
    for relative in PRIVATE_PACKAGES:
        path = root / relative
        data = json.loads(path.read_text())
        if data.get("private") is not True or "version" in data:
            raise ValueError(f"{relative} must be private and have no product version")
        lock = json.loads(path.with_name("package-lock.json").read_text())
        if "version" in lock or "version" in lock["packages"][""]:
            raise ValueError(f"{relative}: remove the build-tool version from the npm lockfile")
    if "version" in json.loads((root / "crates/magistr-desktop/tauri.conf.json").read_text()):
        raise ValueError("Tauri must read the desktop Cargo version")
    return {path: value for path, value in generated.items() if path.read_text() != value}


def lock_errors(root, version):
    names = set(members(root)) | {"magistr-desktop"}
    errors = []
    for relative in ("Cargo.lock", "crates/magistr-desktop/Cargo.lock"):
        path = root / relative
        if not path.is_file():
            errors.append(f"{relative} is missing")
            continue
        found = set()
        for package in toml(path)["package"]:
            if package["name"] in names and "source" not in package:
                found.add(package["name"])
                if package["version"] != version:
                    errors.append(f'{relative}: {package["name"]} is {package["version"]}, expected {version}')
        required = set(members(root)) if relative == "Cargo.lock" else {"magistr-desktop"}
        if missing := required - found:
            errors.append(f"{relative}: missing product packages {sorted(missing)}")
    return errors


def check(root=ROOT):
    version = product_version(root)
    errors = [f"{path.relative_to(root)} differs" for path in generated_changes(root, version)]
    errors.extend(lock_errors(root, version))
    if errors:
        raise ValueError("Release versions drifted; run python3 scripts/version.py sync:\n" + "\n".join(errors))
    return version


def sync(root=ROOT, version=None):
    version = product_version(root) if version is None else version
    changes = generated_changes(root, version)
    for path, content in changes.items():
        path.write_text(content)
    # Cargo owns its lockfiles. Resolve against the existing locks without
    # upgrading external dependencies or requiring registry access.
    for manifest in ("Cargo.toml", DESKTOP):
        subprocess.run(["cargo", "metadata", "--offline", "--format-version", "1",
                        "--manifest-path", str(root / manifest)],
                       cwd=root, stdout=subprocess.DEVNULL, check=True)
    check(root)
    return version, [str(path.relative_to(root)) for path in changes]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="action", required=True)
    commands.add_parser("show", help="Print the canonical release version")
    commands.add_parser("channel", help="Print the npm dist-tag the canonical version publishes on")
    commands.add_parser("check", help="Refuse version or lockfile drift without writing")
    commands.add_parser("sync", help="Regenerate metadata from the canonical version")
    change = commands.add_parser("set", help="Change the version and regenerate all release metadata")
    change.add_argument("version")
    args = parser.parse_args()
    try:
        if args.action == "show":
            print(product_version())
        elif args.action == "channel":
            print(npm_dist_tag(product_version()))
        elif args.action == "check":
            print(f"Release version {check()} is consistent")
        else:
            version, paths = sync(version=args.version if args.action == "set" else None)
            print(json.dumps({"version": version, "updated": paths, "locks": "refreshed by Cargo"}))
    except (ValueError, KeyError, OSError, subprocess.CalledProcessError) as error:
        raise SystemExit(f"Version management failed: {error}") from error


if __name__ == "__main__":
    main()
