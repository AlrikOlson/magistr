#!/usr/bin/env python3
"""Audit the native foundation proof; missed product targets remain explicit."""
import collections
import hashlib
import json
import math
from pathlib import Path
import re
import struct
import tarfile
import tomllib
from urllib.parse import unquote

ROOT = Path(__file__).resolve().parent
PROJECT = ROOT.parent.parent
EVIDENCE = PROJECT / "docs/evidence/desktop-foundation"
ASSETS = PROJECT / "docs/assets/desktop-foundation"


def read(path):
    return json.loads(path.read_text())


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def require(condition, message):
    if not condition:
        raise AssertionError(message)


def p95(values):
    return sorted(values)[math.ceil(len(values) * .95) - 1]


def png(path):
    data = path.read_bytes()
    require(data[:8] == b"\x89PNG\r\n\x1a\n", f"Not a PNG: {path}")
    require(len(data) > 1500, f"Empty capture: {path}")
    return struct.unpack(">II", data[16:24])


def stream(value):
    require(value["received"] == 10000, "Lost fixture events")
    require(value["sequences"] == value["acknowledged_sequences"] == [1250] * 8, "Missing stream acknowledgement")
    require(value["dropped"] == value["delayed"] == 0 and not value["errors"], "Dropped, delayed or reordered fixture events")
    require(len(value["delays_ms"]) == 10000 and max(value["delays_ms"]) <= 50, "Incomplete delivery timing")


def performance(path, selected=False):
    value = read(path / "results.json")
    stream(read(path / "metrics.json"))
    require(value["input_visible_p95_ms"] <= 50, "Input target missed")
    require(value["launch_p95_ms"] <= 2000 and len(value["launch_ms"]) == 5, "Startup target/sample contract")
    samples = value["process_samples"]
    require(math.isclose(value["peak_rss_mib"], max(row["rss_bytes"] for row in samples) / 2**20), "RSS high water does not match raw data")
    require(value["peak_rss_mib"] <= 512, "Memory target missed")
    for sample in samples:
        require(sample["rss_bytes"] == sum(row["rss_bytes"] for row in sample["processes"]), "A process was omitted from aggregate RSS")
        require(len({row["coalition"][0] for row in sample["processes"]}) == 1, "Mixed application coalitions")
    names = {row["name"] for sample in samples for row in sample["processes"]}
    if value["backend"] == "tauri":
        require(all(any(fragment in name for name in names) for fragment in ("WebContent", "WebKit.GPU", "WebKit.Networking")), "WebView helpers missing")
    require(.95 <= value["cpu_calibration"]["ratio"] <= 1.05, "CPU units are uncalibrated")
    for name, metric in (("typing", "input_visible_p95_ms"), ("latency", "workspace_switch_p95_ms")):
        raw = read(path / (name + ".json"))
        require(len(raw["samples"]) == 40, "Incomplete native input sample")
        require(math.isclose(p95([row["display_ms"] for row in raw["samples"]]), value[metric]), "Reported latency does not match visible-pixel samples")
        require(abs(raw["window"][2] - 1280) <= 1 and raw["window"][3] == 832, "Unmatched window geometry")
    motion = read(path / "motion.json")
    require(len(motion["sizes"]) == 60 and all(row["error"] == 0 and all(abs(a-b) <= 1 for a,b in zip(row["actual"],row["requested"])) for row in motion["sizes"]), "Resize was not applied to the real window")
    for label, begin, end in (("scroll", motion["scroll_start_s"], motion["resize_start_s"]), ("resize", motion["resize_start_s"], motion["end_s"])):
        frames = [row["display_s"] for row in motion["frames"] if row["changed"] and begin <= row["display_s"] <= end]
        intervals = [(b-a) * 1000 for a,b in zip(frames, frames[1:])]
        require(len(intervals) > 30 and math.isclose(p95(intervals), value["motion"][label]["visible_interval_p95_ms"]), "Incomplete visible motion evidence")
    if selected:
        first, last = value["stream_cpu_begin"], value["stream_cpu_end"]
        before = {row["pid"]: row["cpu_ns"] for row in first["processes"]}
        require(set(before) <= {row["pid"] for row in last["processes"]}, "An exited helper invalidates endpoint CPU")
        cpu = sum(row["cpu_ns"] - before.get(row["pid"], 0) for row in last["processes"]) / 1e9 / (last["at"] - first["at"]) * 100
        require(math.isclose(cpu, value["stream_cpu_percent_one_core"]), "Incomplete selected CPU accounting")
        print(f"Recorded target misses: idle {value['idle_cpu_percent_one_core']:.2f}% vs 1%; scroll/resize visible cadence {value['motion']['scroll']['visible_interval_p95_ms']:.2f}/{value['motion']['resize']['visible_interval_p95_ms']:.2f} ms vs 16.7 ms. Full WebView CPU frame time remains unavailable.")
    return value


def contrast():
    css = (ROOT / "tauri/ui/style.css").read_text()
    def palette(selector):
        block = re.search(re.escape(selector) + r"\s*\{([^}]+)\}", css).group(1)
        colors = {}
        for name, color in re.findall(r"--([a-z]+):\s*#([a-f0-9]{6}|[a-f0-9]{3})\b", block):
            if len(color) == 3:
                color = "".join(channel * 2 for channel in color)
            colors[name] = tuple(int(color[i:i+2], 16) for i in (0, 2, 4))
        return colors
    def luminance(color):
        channels = [value / 255 for value in color]
        channels = [x/12.92 if x <= .04045 else ((x+.055)/1.055)**2.4 for x in channels]
        return .2126*channels[0] + .7152*channels[1] + .0722*channels[2]
    def ratio(a,b):
        a,b = sorted((luminance(a),luminance(b)))
        return (b+.05)/(a+.05)
    minimum = 100
    for selector in (":root", "body.light"):
        colors = palette(selector)
        for foreground in ("text", "muted", "accent", "wait", "plus", "minus"):
            for background in ("bg", "side", "raised", "selected"):
                value = ratio(colors[foreground],colors[background])
                minimum = min(minimum,value)
                require(value >= 4.5, f"Text contrast: {selector} {foreground}/{background}: {value:.2f}")
        for color in ("plus", "minus"):
            background = tuple(.06*a+.94*b for a,b in zip(colors[color],colors["bg"]))
            require(ratio(colors[color],background) >= 4.5, "Diff text contrast")
        require(ratio(colors["accent"],colors["bg"]) >= 3, "Focus indicator contrast")
    print(f"Foreground contrast minimum {minimum:.2f}:1; focus indicators pass 3:1. Disabled controls and decorative separators are outside these text checks.")


def main():
    config = read(PROJECT / ".magistr/project.json")
    require(config["roadmap_exports"] is False, "Roadmap exports were enabled")
    local = PROJECT / ".codex/config.toml"
    if local.exists():
        settings = tomllib.loads(local.read_text())
        magistr = settings.get("mcp_servers", {}).get("magistr", {})
        require("roadmap_export" in magistr.get("disabled_tools", []), "Export tool deny was removed")
    members = tomllib.loads((ROOT / "Cargo.toml").read_text())["workspace"]["members"]
    require(members == ["workload","tauri"], "More than one renderer is active")
    packages = tomllib.loads((ROOT / "Cargo.lock").read_text())["package"]
    require(not any(row["name"].startswith("gpui") or row["name"] == "magistr-tui" for row in packages), "Unselected/terminal renderer dependency")
    require(not list((ROOT / "gpui").rglob("*.rs")), "Unselected prototype sources remain active")

    manifest = read(EVIDENCE / "manifest.json")
    current = {str(path.relative_to(PROJECT)) for root in (EVIDENCE,ASSETS) for path in root.rglob("*") if path.is_file() and path.name != "manifest.json"}
    require(current == set(manifest), "Evidence manifest does not cover the retained artifacts")
    for path, expected in manifest.items():
        require(digest(PROJECT/path) == expected, f"Evidence changed: {path}")
    build = read(EVIDENCE / "build.json")
    archive = EVIDENCE / "comparison-source.tar.gz"
    require(digest(archive) == build["archive_sha256"], "Matched source archive changed")
    with tarfile.open(archive) as tar:
        for source, expected in build["sources_sha256"].items():
            data = tar.extractfile("desktop-foundation/"+source).read()
            require(hashlib.sha256(data).hexdigest() == expected, f"Unmatched compared source: {source}")
    selected = read(EVIDENCE / "selected-validation/build.json")
    for path, expected in selected["source_sha256"].items():
        require(digest(PROJECT/path) == expected, f"Selected evidence predates source: {path}")

    for candidate in ("tauri","gpui"):
        for number in range(3):
            performance(EVIDENCE / "performance" / candidate / ("run-"+str(number)))
        journey = read(EVIDENCE / "journeys" / candidate / "journeys.json")
        failures = [row["name"] for row in journey["checks"] if not row["passed"]]
        require(len(journey["checks"]) == (15 if candidate == "tauri" else 16), "Native journey is incomplete")
        if candidate == "tauri":
            require(journey["all_passed"] and not failures, "Selected journey failed")
            require(journey["ime_automation"]["status"] == "unavailable", "Unreliable IME automation was claimed as passed")
        else:
            require(failures == ["Shift Return inserts a line without sending"], "Unrecorded candidate limitation")
        ime = read(EVIDENCE / "journeys" / candidate / "ime.json")
        value = [row.get("AXValue") for row in ime["committed"] if row.get("AXRole") in ("AXTextArea","AXTextField")]
        preedit = [row.get("AXValue") for row in ime["preedit"] if row.get("AXRole") in ("AXTextArea","AXTextField")]
        require(value == ["日本"] and preedit == ["にほn"], "Native Japanese composition was not observed")
        require(ime["actual_source"] == "com.apple.inputmethod.Kotoeri.RomajiTyping.Japanese", "A real input method was not selected")
        walk = read(EVIDENCE / "voiceover" / candidate / "walk.json")
        require(len(walk["steps"]) == 28 and all(row["voiceover_pid"] > 0 and "actual VoiceOver" in row["observation"] and row["caption"] for row in walk["steps"]), "Reader evidence is synthetic or incomplete")
        stream(read(EVIDENCE / "voiceover" / candidate / "app-metrics.json"))
        for name in ("dark","light","narrow","multilingual","workspace-picker","changes","roadmap","ime"):
            width,height = png(ASSETS / candidate / (name+".png"))
            require(width >= 1800 and height >= 1600, "Missing native window capture")
        require(digest(ASSETS / candidate / "dark.png") != digest(ASSETS / candidate / "light.png"), "Themes have identical captures")
    require(read(EVIDENCE / "voiceover/tauri/walk.json")["activated_start"], "Reader did not activate the selected control")
    for file in ASSETS.glob("*/voiceover-*.png"):
        width,height = png(file)
        require(300 <= width <= 2000 and 20 <= height <= 600, "Reader caption capture included an unrelated screen")
    roles = collections.Counter(row.get("AXRole") for row in read(EVIDENCE / "journeys/tauri/multilingual.json"))
    require(all(roles[role] for role in ("AXHeading","AXList","AXTable","AXLink","AXTextArea")), "Rich-text semantics missing")

    icon = ROOT / "assets/icon.png"
    require(png(icon) == (512,512) and digest(icon) == digest(PROJECT / "apps/website/public/icon-512.png"), "Magistr icon changed")
    contrast()
    performance(EVIDENCE / "selected-validation", selected=True)
    for name in ("docs/architecture/desktop-foundation.md","docs/architecture/overview.md","docs/architecture/desktop.md","docs/architecture/system-contracts.md","docs/evidence/desktop-foundation/README.md","benchmarks/desktop-foundation/README.md"):
        path = PROJECT / name
        for destination in re.findall(r"\]\(([^)]+)\)",path.read_text()):
            destination = unquote(destination.split("#")[0])
            if not destination or "://" in destination:
                continue
            require((path.parent/destination).exists(), f"Architecture/evidence link missing: {name} -> {destination}")
    for name in ("overview.md","desktop.md","system-contracts.md"):
        require("desktop-foundation.md" in (PROJECT/"docs/architecture"/name).read_text(), "Maintained architecture index omits foundation")
    print(f"Foundation artifact audit passed: {len(manifest)} immutable/selected artifacts, both native candidates, actual reader/IME, contrast, icon, screenshots and maintained index.")


if __name__ == "__main__":
    main()
