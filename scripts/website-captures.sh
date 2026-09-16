#!/usr/bin/env bash
# website-captures.sh — regenerate the website's terminal captures from the
# synthetic Harbor fixture and rewrite the manifest beside them.
#
# The captures are the real terminal renderer's frames of a sample project
# (apps/website/fixtures/harbor.json); the project's own roadmap is never
# read. Run this after an intentional change to the terminal interface, then
# review the four images and commit them with the manifest.
#
# Usage:
#   ./scripts/website-captures.sh

set -euo pipefail

PROJECT_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
FIXTURE="$PROJECT_ROOT/apps/website/fixtures/harbor.json"
OUT="$PROJECT_ROOT/apps/website/public/screenshots"

cd "$PROJECT_ROOT"
cargo run -q -p magistr-tui --example website_capture -- "$FIXTURE" "$OUT"

python3 - "$OUT" "$FIXTURE" <<'PY'
import hashlib, json, struct, sys, datetime, pathlib
out, fixture = pathlib.Path(sys.argv[1]), pathlib.Path(sys.argv[2])
images = []
for name in ["terminal-overview-dark.png", "terminal-overview-light.png",
             "terminal-roadmap-dark.png", "terminal-roadmap-light.png",
             "terminal-session-dark.png", "terminal-session-light.png"]:
    data = (out / name).read_bytes()
    width, height = struct.unpack(">II", data[16:24])
    images.append({"file": name, "sha256": hashlib.sha256(data).hexdigest(),
                   "width": width, "height": height})
manifest = {
    "renderer": "magistr-tui::screenshot::frame",
    "surface": "Terminal",
    "profile": "Kitty",
    "source_commit": "working-tree",
    "captured_on": datetime.date.today().isoformat(),
    "state": f"Rendered from {fixture.relative_to(out.parents[3])}",
    "columns": 132,
    "rows": 42,
    "annotations": False,
    "images": images,
}
(out / "captures.json").write_text(json.dumps(manifest, indent=2) + "\n")
print("wrote", out / "captures.json")
PY
