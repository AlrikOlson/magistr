#!/usr/bin/env bash
# Parse with nextest itself; never compile a test or turn a failure into a skip.
set -euo pipefail
exec python3 -B "$(cd "$(dirname "$0")" && pwd)/nextest-config-gate.py" "$@"
