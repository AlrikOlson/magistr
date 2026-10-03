"""Exercise only this checkout's Magistr installation through Claude's plugin manager.

Opt-in because this temporarily removes/reinstalls the user's Magistr plugin.
It never edits settings, caches, managed files, native state, or source itself.
"""
import argparse
import hashlib
import json
from pathlib import Path
import shutil
import subprocess

ROOT = Path(__file__).resolve().parents[1]
PLUGIN = "magistr@magistr"
CLAUDE = shutil.which("claude")


def run(*args):
    result = subprocess.run(
        [CLAUDE, *args], cwd=ROOT, text=True, capture_output=True,
        timeout=90, check=False,
    )
    if result.returncode:
        raise RuntimeError(f"Claude command failed ({result.returncode}): "
                           f"{' '.join(args)}\n{result.stdout}\n{result.stderr}")
    return json.loads(result.stdout)


def digest(file):
    return hashlib.sha256(file.read_bytes()).hexdigest() if file.exists() else None


def settings():
    file = Path.home() / ".claude/settings.json"
    value = json.loads(file.read_text(encoding="utf-8")) if file.exists() else {}
    value.get("enabledPlugins", {}).pop(PLUGIN, None)
    value.get("extraKnownMarketplaces", {}).pop("magistr", None)
    if value.get("extraKnownMarketplaces") == {}:
        value.pop("extraKnownMarketplaces", None)
    if value.get("enabledPlugins") == {}:
        value.pop("enabledPlugins", None)
    return value


def plugins():
    return run("plugin", "list", "--json")


def marketplaces():
    return run("plugin", "marketplace", "list", "--json")


def other_plugins():
    return sorted((p["id"], p["scope"], p["version"], p["enabled"],
                   p["installPath"]) for p in plugins() if p["id"] != PLUGIN)


def other_markets():
    return sorted(json.dumps(p, sort_keys=True) for p in marketplaces()
                  if p["name"] != "magistr")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--exercise-installation", action="store_true", required=True)
    parser.parse_args()
    if not CLAUDE:
        raise RuntimeError("Claude Code is not installed.")
    market = next((p for p in marketplaces() if p["name"] == "magistr"), None)
    if not market or market.get("source") != "directory" or Path(market["path"]).resolve() != ROOT:
        raise RuntimeError("Register this checkout with claude plugin marketplace add ./ first. "
                           "A different marketplace named magistr will not be changed.")
    protected = [ROOT / p for p in (
        ".mcp.json", ".claude/settings.json", "CLAUDE.md", "AGENTS.md",
        ".codex/config.toml", ".magistr/project.json",
        "plugins/magistr/README.md",
    )]
    hashes = {p: digest(p) for p in protected}
    baseline_settings, baseline_plugins, baseline_markets = settings(), other_plugins(), other_markets()

    def preserved():
        assert settings() == baseline_settings, "Unrelated user settings changed."
        assert other_plugins() == baseline_plugins, "Another plugin changed."
        assert other_markets() == baseline_markets, "Another marketplace changed."
        assert {p: digest(p) for p in protected} == hashes, "Protected checkout files changed."

    def installed(enabled=True):
        item = next(p for p in plugins() if p["id"] == PLUGIN and p["scope"] == "user")
        version = json.loads((ROOT / "plugins/magistr/.claude-plugin/plugin.json").read_text())["version"]
        assert item["version"] == version and item["enabled"] == enabled, item
        cache = Path(item["installPath"])
        for file in (".claude-plugin/plugin.json", "hooks/hooks.json",
                     "hooks/register.js", "hooks/wire.js", "hooks/view.js",
                     "skills/status/SKILL.md"):
            assert digest(cache / file) == digest(ROOT / "plugins/magistr" / file), file
        return item

    run("plugin", "marketplace", "update", "magistr", "--json")
    run("plugin", "update", PLUGIN, "--scope", "user", "--json")
    installed(); preserved()
    print("PASS installed upgrade: current version and exact runtime files")
    run("plugin", "disable", PLUGIN, "--scope", "user", "--json")
    installed(False); preserved()
    run("plugin", "enable", PLUGIN, "--scope", "user", "--json")
    installed(); preserved()
    print("PASS disable and enable: only Magistr changed")
    try:
        run("plugin", "uninstall", PLUGIN, "--scope", "user", "--json")
        assert not any(p["id"] == PLUGIN for p in plugins())
        run("plugin", "marketplace", "remove", "magistr", "--json")
        assert not any(p["name"] == "magistr" for p in marketplaces())
        preserved()
        print("PASS removal: checkout, registrations, user settings and other plugins preserved")
    finally:
        # Restore this integration even when a preservation assertion fails.
        # Other settings remain evidence; they are never rewritten as a repair.
        if not any(p["name"] == "magistr" for p in marketplaces()):
            run("plugin", "marketplace", "add", "./", "--json")
        if not any(p["id"] == PLUGIN for p in plugins()):
            run("plugin", "install", PLUGIN, "--scope", "user", "--json")
    installed(); preserved()
    print("PASS repair: source re-registered and exact version reinstalled")
    print("Installation verified. This does not prove that the host can load or draw the Mod.")


if __name__ == "__main__":
    main()
