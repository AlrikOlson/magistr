#!/usr/bin/env python3
"""Check the resolved workflow source and retained build/test inputs, read-only."""

import json
from pathlib import Path
import subprocess
import sys
import tomllib


REVISION = "8c10c1fcfb479e90b33c59502eabd92586cf0761"
SUBSYSTEM = Path("subsystems/workflow")
CRATE = Path("crates/magistr-workflow")
PACKAGE = "magistr-workflow"
RETIRED = ("src/main.rs", "src/cli", "build.rs", "examples", "skill-clients", "skills", "README.md")


def check(root):
    errors = []

    def require(condition, message):
        if not condition:
            errors.append(message)

    def required_file(relative):
        path = root / relative
        require(path.is_file() and path.stat().st_size > 0,
                f"missing or empty imported input: {relative}")
        require(path.resolve().is_relative_to(root),
                f"imported input escapes the repository: {relative}")

    # These are source inputs, not installed copies or files in an author's home.
    for relative in (
        "LICENSE", "PROVENANCE.md", ".githooks/commit-msg", "contract/unified-record-envelope.schema.json",
        "contract/signal-envelope.schema.json", "shared/name-derivation.cases.json",
        "shared/region-unplaced.json", "docs/HARNESSES.md", "docs/CLI.md",
    ):
        required_file(SUBSYSTEM / relative)
    for relative in (
        "Cargo.toml", "src/lib.rs",
        "src/think/mod.rs", "src/ship/mod.rs", "src/roadmap/mod.rs", "src/signal/mod.rs",
        "src/tracker/host/mod.rs",
        "tests/family_stores_durability.rs", "tests/workflow_e2e.rs",
        "tests/unified_service.rs",
        "tests/tool_surface.txt", "tests/fixtures/projects_v2_fields.json",
    ):
        required_file(CRATE / relative)
    # The standalone surfaces retired by arch-14f4 stay deleted: no binary, no
    # CLI, no skill embedding, no examples. The library is release-coupled to the published CLI.
    for relative in RETIRED:
        require(not (root / CRATE / relative).exists(), f"retired standalone surface is back: {CRATE / relative}")
    manifest_text = (root / CRATE / "Cargo.toml").read_text()
    require("[[bin]]" not in manifest_text, "the workflow crate must not declare a binary")
    manifest = tomllib.loads(manifest_text)
    require(manifest["package"]["name"] == PACKAGE, f"the workflow package must be named {PACKAGE}")
    require(manifest["package"].get("publish") is not False, "the workflow library must be publishable with the CLI")
    require(manifest["lib"]["name"] == "magistr_workflow", "the workflow library must carry the Magistr name")
    license_path = root / SUBSYSTEM / "LICENSE"
    if license_path.is_file():
        license_text = license_path.read_text()
        require("MIT License" in license_text and "Copyright (c) 2026 Alrik Olson" in license_text,
                "workflow LICENSE must retain the upstream license and attribution")
    provenance = root / SUBSYSTEM / "PROVENANCE.md"
    if provenance.is_file():
        require(REVISION in provenance.read_text(), "workflow provenance must name the audited revision")

    project = json.loads((root / ".magistr/project.json").read_text())
    require(project.get("roadmap_exports") is False, "roadmap_exports must remain false")
    # Generated agent setup is local/ignored (`magistr init`); check each client
    # when present. The tracked owner of the policy is roadmap_exports above.
    settings_path = root / ".claude/settings.json"
    if settings_path.is_file():
        settings = json.loads(settings_path.read_text())
        require("mcp__magistr__roadmap_export" in settings.get("permissions", {}).get("deny", []),
                "the local Claude export-tool denial must remain enabled")
    codex_path = root / ".codex/config.toml"
    if codex_path.is_file():
        codex = tomllib.loads(codex_path.read_text())
        magistr = codex.get("mcp_servers", {}).get("magistr")
        if magistr is not None:
            require("roadmap_export" in magistr.get("disabled_tools", []),
                    "the local Codex Magistr export-tool denial must remain enabled")
    for filename in ("ROADMAP.md", "ROADMAP.json"):
        require(not (root / filename).exists(), f"native roadmap must not be exported to {filename}")

    lock = tomllib.loads((root / "Cargo.lock").read_text())
    locked = [package for package in lock.get("package", []) if package["name"] == PACKAGE]
    require(len(locked) == 1 and "source" not in locked[0],
            f"Cargo.lock must contain exactly one local {PACKAGE} package")
    require(not any(package["name"] == "think-and-ship" for package in lock.get("package", [])),
            "the upstream package name must not resolve anywhere in the lockfile")
    result = subprocess.run(
        ["cargo", "metadata", "--locked", "--format-version", "1"],
        cwd=root, capture_output=True, text=True, check=False,
    )
    if result.returncode:
        errors.append(f"locked Cargo metadata failed: {result.stderr.strip()}")
        return errors
    metadata = json.loads(result.stdout)
    require(Path(metadata["workspace_root"]).resolve() == root,
            "Cargo metadata must describe the implementation worktree")
    packages = metadata["packages"]
    workflow = [package for package in packages if package["name"] == PACKAGE]
    require(len(workflow) == 1, f"resolved graph must contain exactly one {PACKAGE} package")
    expected = (root / CRATE / "Cargo.toml").resolve()
    if len(workflow) == 1:
        package = workflow[0]
        require(package["source"] is None, f"{PACKAGE} must not resolve from a registry or Git")
        require(Path(package["manifest_path"]).resolve() == expected,
                f"{PACKAGE} must resolve from its repository-local crate")
        require(package["id"] in metadata["workspace_members"],
                f"{PACKAGE} must participate in workspace checks as a first-party member")
        require(not any(target["kind"] == ["bin"] or "custom-build" in target["kind"] for target in package["targets"]),
                f"{PACKAGE} must build no binary and run no build script")
        require(package["license"] == "MIT", "the owned package must retain its MIT license")
        nodes = {node["id"]: node for node in metadata["resolve"]["nodes"]}
        for caller in ("magistr-engine", "magistr-cli"):
            callers = [item for item in packages if item["name"] == caller]
            require(len(callers) == 1, f"expected one {caller} package")
            if len(callers) == 1:
                deps = nodes[callers[0]["id"]]["deps"]
                require(any(dep["pkg"] == package["id"] for dep in deps),
                        f"{caller} must depend on the owned workflow package")
    # One release-coupled version for every first-party crate.
    members = [package for package in packages if package["id"] in metadata["workspace_members"]]
    versions = {package["version"] for package in members}
    require(len(versions) == 1, f"first-party crates must share one version; found {sorted(versions)}")
    for package in packages:
        if package["source"] is None:
            manifest = Path(package["manifest_path"]).resolve()
            require(manifest.is_relative_to(root),
                    f"local package {package['name']} resolves outside this repository: {manifest}")
        if package["id"] in metadata["workspace_members"]:
            for dependency in package["dependencies"]:
                if dependency.get("path"):
                    path = Path(dependency["path"])
                    require(path.resolve().is_relative_to(root),
                            f"{package['name']} has an external local dependency: {path}")
    return errors


if __name__ == "__main__":
    try:
        failures = check(Path(sys.argv[1]).resolve())
    except (OSError, ValueError, KeyError) as error:
        failures = [str(error)]
    for failure in failures:
        print(f"ERROR: workflow-source — {failure}", file=sys.stderr)
    if failures:
        sys.exit(1)
    print("OK: workflow source is one Magistr-named workspace member without its standalone surfaces; imported inputs and native-only roadmap policy retained")
