#!/usr/bin/env bash
# Current headless layout, palette, icon, module index and recorded native evidence.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"
python3 scripts/interface-index-gate.py
python3 scripts/desktop-visual-gate.py
