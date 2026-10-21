#!/usr/bin/env bash
# Hash each file once, then check cross-tree clones with the locked jscpd tool.
set -euo pipefail
exec python3 -B "$(cd "$(dirname "$0")" && pwd)/lint-duplication.py" "$@"
