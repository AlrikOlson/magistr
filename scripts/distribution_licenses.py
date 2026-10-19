#!/usr/bin/env python3
"""Collect exact dependency notices; refresh missing archive notices from pinned upstream commits."""
import argparse
import hashlib
import json
from pathlib import Path
import shutil
import subprocess
from urllib.request import Request, urlopen

ROOT = Path(__file__).resolve().parents[1]
SUPPLEMENT = ROOT / "npm/magistr/licenses"
PREFIXES = ("license", "copying", "notice", "copyright")
# The 1.0.0 crate metadata names an unavailable organization path. Its source
# commit remains available at the maintained upstream repository.
REPOSITORIES = {"tree-sitter-gleam": "https://github.com/gleam-lang/tree-sitter-gleam"}


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def dependency_closure(meta):
    packages = {p["id"]: p for p in meta["packages"]}
    nodes = {n["id"]: n for n in meta["resolve"]["nodes"]}
    selected = set()

    def visit(identity):
        if identity in selected:
            return
        selected.add(identity)
        for dep in nodes[identity]["deps"]:
            if any(kind["kind"] != "dev" for kind in dep["dep_kinds"]):
                visit(dep["pkg"])

    visit(next(p["id"] for p in meta["packages"] if p["name"] == "magistr-cli"))
    return sorted((packages[i] for i in selected), key=lambda p: (p["name"], p["version"]))


def local_notices(package):
    base = Path(package["manifest_path"]).parent
    # Nested notices matter for embedded native libraries and generated parsers.
    return sorted(f for f in base.rglob("*") if f.is_file()
                  and f.name.lower().startswith(PREFIXES)
                  and f.suffix.lower() not in (".rs", ".py", ".js", ".ts", ".c", ".h", ".cpp"))


def vcs(package):
    path = Path(package["manifest_path"]).parent / ".cargo_vcs_info.json"
    return json.loads(path.read_text())["git"]["sha1"] if path.exists() else None


def collect(meta, destination):
    manifest = json.loads((SUPPLEMENT / "manifest.json").read_text())
    records = []
    for package in dependency_closure(meta):
        base = Path(package["manifest_path"]).parent
        identity = package["name"] + "@" + package["version"]
        sources = [(f, str(f.relative_to(base)), None) for f in local_notices(package)]
        if not sources:
            entry = manifest.get(identity)
            if not entry or entry["commit"] != vcs(package):
                raise SystemExit("Missing pinned dependency notices: " + identity)
            for notice in entry["notices"]:
                path = SUPPLEMENT / notice["file"]
                if digest(path) != notice["sha256"]:
                    raise SystemExit("Dependency notice checksum mismatch: " + str(path))
                sources.append((path, notice["file"], notice["source"]))
        copied = []
        for path, relative, source in sources:
            name = package["name"] + "-" + package["version"] + "-" + relative.replace("/", "__")
            shutil.copy2(path, destination / name)
            copied.append({"file": name, "sha256": digest(path), "source": source})
        records.append({"name": package["name"], "version": package["version"],
                        "license": package["license"], "source": package["source"],
                        "notices": copied})
    return records


def fetch(url):
    with urlopen(Request(url, headers={"User-Agent": "magistr-package-notices"}), timeout=30) as response:
        return response.read()


def refresh(meta):
    """Explicit maintenance step; packaging itself never fetches mutable license text."""
    missing = [p for p in dependency_closure(meta) if not local_notices(p)]
    repos = {}
    for p in meta["packages"]:
        if p.get("repository") and vcs(p):
            repos.setdefault(vcs(p), set()).add(p["repository"].split("/tree/")[0].rstrip("/"))
    groups = {}
    for package in missing:
        commit = vcs(package)
        repository = (REPOSITORIES.get(package["name"]) or package.get("repository") or "").split("/tree/")[0].rstrip("/")
        if not repository and len(repos.get(commit, [])) == 1:
            repository = next(iter(repos[commit]))
        if not commit or not repository.startswith("https://github.com/"):
            raise SystemExit("Review dependency notice origin manually: " + package["name"])
        groups.setdefault((repository, commit), []).append(package)
    SUPPLEMENT.mkdir(parents=True, exist_ok=True)
    manifest = {}
    for (repository, commit), packages in sorted(groups.items()):
        slug = repository.removeprefix("https://github.com/")
        listing = json.loads(fetch("https://api.github.com/repos/" + slug + "/contents?ref=" + commit))
        files = [f["name"] for f in listing if f["type"] == "file"
                 and f["name"].lower().startswith(PREFIXES)]
        if not files:
            raise SystemExit("No root notice at pinned upstream revision: " + repository + " " + commit)
        notices = []
        for name in sorted(files):
            url = "https://raw.githubusercontent.com/" + slug + "/" + commit + "/" + name
            relative = slug.replace("/", "__") + "/" + commit + "/" + name
            path = SUPPLEMENT / relative
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_bytes(fetch(url))
            notices.append({"file": relative, "sha256": digest(path), "source": url})
        for p in packages:
            manifest[p["name"] + "@" + p["version"]] = {"commit": commit, "notices": notices}
        print(slug, commit[:12], len(packages), "packages;", len(notices), "notices", flush=True)
    (SUPPLEMENT / "manifest.json").write_text(json.dumps(manifest, indent=2) + "\n")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("action", choices=["refresh"])
    parser.parse_args()
    metadata = json.loads(subprocess.check_output(
        ["cargo", "metadata", "--locked", "--filter-platform", "aarch64-apple-darwin", "--format-version", "1"],
        cwd=ROOT, text=True))
    refresh(metadata)
