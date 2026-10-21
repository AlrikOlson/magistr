#!/usr/bin/env python3
"""Resolve the affected CI contract; unknown inputs always select broad checks."""
import argparse
from datetime import datetime, timedelta, timezone
import json
import os
from pathlib import Path
import subprocess
import tomllib
import urllib.parse
import urllib.request

ROOT = Path(__file__).resolve().parent.parent
JOBS = ("rust", "website", "desktop", "package", "contracts", "visual")


def git(*args, root=ROOT):
    return subprocess.check_output(["git", *args], cwd=root, text=True).strip()


def graph(root=ROOT):
    """Include normal, build, dev and target dependencies, including workspace aliases."""
    workspace = tomllib.loads((root / "Cargo.toml").read_text())
    inherited = workspace.get("workspace", {}).get("dependencies", {})
    manifests = [root / member / "Cargo.toml" for member in workspace["workspace"]["members"]]
    manifests.append(root / "crates/magistr-desktop/Cargo.toml")
    packages, locations = {}, {}
    for manifest in manifests:
        data = tomllib.loads(manifest.read_text())
        name = data["package"]["name"]
        locations[manifest.parent.relative_to(root).as_posix()] = name
        tables = [data, *data.get("target", {}).values()]
        dependencies = set()
        for table in tables:
            for key in ("dependencies", "dev-dependencies", "build-dependencies"):
                for alias, value in table.get(key, {}).items():
                    if isinstance(value, dict) and value.get("workspace"):
                        value = inherited.get(alias, value)
                    dependencies.add(value.get("package", alias) if isinstance(value, dict) else alias)
        packages[name] = dependencies
    return packages, locations


def affected(paths, packages, locations, full=False):
    plan = {key: False for key in JOBS}
    changed = set()
    for path in paths:
        if path.startswith(("docs/", "apps/website/", "examples/support-inbox/")) or path in ("README.md", "CHANGELOG.md"):
            plan["website"] = True
            plan["visual"] |= path.startswith(("docs/evidence/", "docs/assets/"))
        elif path.startswith("crates/"):
            location = next((item for item in locations if path.startswith(item + "/")), None)
            if location is None:
                full = True
            else:
                changed.add(locations[location])
                if path.endswith("/Cargo.toml"):
                    full = True
        elif path.startswith(("scripts/", ".github/", ".config/", ".cargo/", ".magistr/",
                              ".agents/", ".claude/", ".codex/", "reference/")):
            full = True
        else:
            # New build inputs, manifests, tools and unclassified files are never silently omitted.
            full = True
    selected = set(changed)
    while True:
        consumers = {name for name, deps in packages.items() if deps & selected}
        if consumers <= selected:
            break
        selected |= consumers
    desktop = "magistr-desktop"
    plan["desktop"] = desktop in selected
    plan["rust_packages"] = sorted(selected - {desktop})
    plan["rust"] = bool(plan["rust_packages"])
    plan["contracts"] = bool(selected)
    if full:
        plan.update({key: True for key in JOBS})
        plan["rust_packages"] = sorted(set(packages) - {desktop})
    plan["full"] = full
    # Broad suites use build-once partitioning; a small crate check stays on one worker.
    plan["partitions"] = [1, 2, 3, 4] if len(plan["rust_packages"]) > 3 else [1]
    plan["partition_count"] = len(plan["partitions"])
    plan["reuse"] = False
    return plan


def reusable(run, sha, now=None):
    now = now or datetime.now(timezone.utc)
    updated = datetime.fromisoformat(run["updated_at"].replace("Z", "+00:00"))
    return (run.get("head_sha") == sha and run.get("event") == "push"
            and run.get("head_branch", "").startswith("verify/")
            and run.get("path") == ".github/workflows/public-release.yml"
            and run.get("status") == "completed" and run.get("conclusion") == "success"
            and timedelta() <= now - updated <= timedelta(hours=24))


def previous_verification(sha):
    """Reuse only a successful eligible push of this exact workflow/source revision."""
    token, repository = os.environ.get("GH_TOKEN"), os.environ.get("GITHUB_REPOSITORY")
    if not token or not repository:
        return None
    query = urllib.parse.urlencode({"head_sha": sha, "event": "push", "status": "success",
                                    "per_page": 100})
    url = f"https://api.github.com/repos/{repository}/actions/workflows/public-release.yml/runs?{query}"
    request = urllib.request.Request(url, headers={"Authorization": f"Bearer {token}",
                                                  "Accept": "application/vnd.github+json"})
    try:
        with urllib.request.urlopen(request, timeout=20) as response:
            runs = json.load(response)["workflow_runs"]
    except (OSError, ValueError, KeyError):
        return None  # An unavailable cache must run the real checks.
    return next((run["html_url"] for run in runs if reusable(run, sha)), None)


def validate(plan, needs):
    if needs.get("plan", {}).get("result") != "success":
        raise ValueError("Verification planning failed")
    if needs.get("policy", {}).get("result") != "success":
        raise ValueError("Verification policy tests failed")
    for kind in JOBS:
        names = ("rust-build", "rust-tests") if kind == "rust" else (kind,)
        selected = plan[kind] and not plan["reuse"]
        expected = "success" if selected else "skipped"
        for name in names:
            actual = needs.get(name, {}).get("result")
            if actual != expected:
                raise ValueError(f"{name}: expected {expected}, received {actual}")
    if plan["reuse"] and not plan.get("verified_run"):
        raise ValueError("A reused plan has no successful exact-commit verification")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--assert-needs", action="store_true")
    parser.add_argument("--base")
    parser.add_argument("--full", action="store_true")
    parser.add_argument("--out", default="target/ci-plan.json")
    args = parser.parse_args()
    if args.assert_needs:
        validate(json.loads(os.environ["CI_PLAN"]), json.loads(os.environ["CI_NEEDS"]))
        print("All selected checks passed; unselected checks were explicitly accounted for.")
        return
    event = os.environ.get("GITHUB_EVENT_NAME", "")
    payload = json.loads(Path(os.environ["GITHUB_EVENT_PATH"]).read_text()) if os.environ.get("GITHUB_EVENT_PATH") else {}
    head = git("rev-parse", "HEAD")
    base = args.base
    full = args.full or event in ("schedule", "workflow_dispatch") or os.environ.get("GITHUB_REF", "").startswith("refs/tags/")
    if not base and not full:
        if event == "pull_request":
            base = git("merge-base", head, payload["pull_request"]["base"]["sha"])
        elif os.environ.get("GITHUB_REF") == "refs/heads/main":
            base = payload.get("before")
        else:
            base = git("merge-base", head, "origin/main")
    if not base or not base.strip("0"):
        full = True
    paths = git("diff", "--name-only", "--no-renames", base, head).splitlines() if not full else []
    packages, locations = graph()
    plan = affected(paths, packages, locations, full)
    plan.update({"source": head, "base": base, "changed": paths})
    if event == "push" and os.environ.get("GITHUB_REF") == "refs/heads/main":
        verified = previous_verification(head)
        if verified:
            plan.update({"reuse": True, "verified_run": verified})
    output = Path(args.out)
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_text(json.dumps(plan, indent=2) + "\n")
    summary = {key: value for key, value in plan.items() if key != "changed"}
    summary["changed_count"] = len(paths)
    if os.environ.get("GITHUB_OUTPUT"):
        with open(os.environ["GITHUB_OUTPUT"], "a") as stream:
            for key, value in summary.items():
                stream.write(f"{key}={json.dumps(value, separators=(',', ':'))}\n")
            stream.write(f"plan={json.dumps(summary, separators=(',', ':'))}\n")
    print(json.dumps(summary, indent=2))


if __name__ == "__main__":
    main()
