#!/usr/bin/env bash
# Check screenshot and animation goldens; reject unreviewed *.new.png files.
# For intended visual changes, review the images and regenerate them with
# MAGISTR_GOLDEN_UPDATE=1.

set -euo pipefail

PROJECT_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
GOLDEN_DIRS=(
    "$PROJECT_ROOT/crates/magistr-cockpit/tests/golden"
    "$PROJECT_ROOT/crates/magistr-tui/tests/golden"
    "$PROJECT_ROOT/crates/magistr-gui2/tests/golden"
)

(cd "$PROJECT_ROOT" && cargo nextest run -p magistr-cockpit -p magistr-tui -p magistr-gui2 -E 'test(screenshot) | test(shot_) | test(golden)')
candidates="$(find "${GOLDEN_DIRS[@]}" -name '*.new.png' 2>/dev/null || true)"
if [[ -n "$candidates" ]]; then
    echo ""
    echo "ERROR: unreviewed screenshot candidates:"
    echo "$candidates"
    echo "Compare each with its golden; re-bless with MAGISTR_GOLDEN_UPDATE=1 or fix the regression."
    exit 1
fi

echo "OK: every cockpit screenshot and filmstrip matches its golden"
