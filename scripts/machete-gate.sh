#!/usr/bin/env bash
# Check every workspace member for unused dependencies.

set -euo pipefail

PROJECT_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
SKIP="${MACHETE_SKIP:-}"

if ! command -v cargo-machete >/dev/null 2>&1; then
    echo "ERROR: cargo-machete is not installed (cargo install cargo-machete)"
    exit 1
fi

manifests="$(cd "$PROJECT_ROOT" && cargo metadata --no-deps --format-version 1 \
    | python3 -c '
import json, sys
skip = set(sys.argv[1].split())
meta = json.load(sys.stdin)
for pkg in meta["packages"]:
    if pkg["name"] not in skip:
        print(pkg["manifest_path"])
' "$SKIP")"

# shellcheck disable=SC2086 # one manifest path per word
(cd "$PROJECT_ROOT" && cargo machete $manifests)
