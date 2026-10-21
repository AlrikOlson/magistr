#!/usr/bin/env bash
# Native ownership rules share one source scan; see native-write-path-gate.py.
set -euo pipefail
exec python3 -B "$(cd "$(dirname "$0")" && pwd)/native-write-path-gate.py" "$@"
