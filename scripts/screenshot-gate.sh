#!/usr/bin/env bash
# Current native evidence must match the reviewed images and interface sources.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"
python3 scripts/interface-index-gate.py
python3 scripts/desktop-visual-gate.py
