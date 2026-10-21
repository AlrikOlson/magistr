#!/usr/bin/env bash
# Run all structure rules over one cached source scan.
set -euo pipefail
exec python3 -B "$(cd "$(dirname "$0")" && pwd)/lint-structure.py" "$@"
