#!/usr/bin/env bash
# Smoke the shipping native workspace artifact: the release-profile `magistr`
# binary serving a disposable workspace over stdio with nothing beside it (a
# PATH holding only `echo` — no Bun, Node, N-API addon or addon-to-WASM
# fallback reachable), through initialize, tools/list and every core tool
# once. The test refuses a debug-profile binary, so this is never a proxy.
#
# Usage: ./scripts/workspace-release-smoke.sh

set -euo pipefail

PROJECT_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$PROJECT_ROOT"

if ! command -v cargo >/dev/null 2>&1; then
    echo "ERROR: cargo is not on PATH — the release smoke cannot run"
    exit 1
fi

cargo nextest run --locked --release -p magistr-cli \
    -E 'binary(workspace_release_smoke)' --run-ignored only

echo "OK: the release binary serves every core workspace tool with no runtime beside it"
