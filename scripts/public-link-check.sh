#!/usr/bin/env bash
# One public-readiness owner checks status, deployed content and reporting routes.
set -euo pipefail
exec python3 -B "$(dirname "$0")/public_link_check.py" "$@"
