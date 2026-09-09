#!/usr/bin/env bash
# Compare native fixture renders with reference captures and layout masks.
# Unimplemented views are skipped; differences produce *.diff.png files.
# This optional check measures unfinished desktop views.

set -uo pipefail

PROJECT_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$PROJECT_ROOT"

if ! command -v cargo >/dev/null 2>&1; then
    echo "SKIPPED: cargo is not on PATH — the mock parity gate was not run"
    exit 0
fi

if ! cargo nextest run -p magistr-gui2 --no-run >/dev/null 2>&1; then
    echo "SKIPPED: the magistr-gui2 test binaries do not build — see the check gate"
    exit 0
fi

cargo nextest run -p magistr-gui2 --run-ignored all --no-fail-fast --no-capture \
    -E 'binary(mock_parity) | test(no_role_is_both_a_control_and_labelled)'
status=$?

if [[ $status -ne 0 ]]; then
    echo ""
    echo "FAIL: mock-parity — a native view is over its capture's threshold or a design prohibition was found (table above)."
    echo "Open tests/mocks/<medium>-NN.diff.png beside its capture; the fix is the view, never the mask (a mask rect names a LIVE region and says why)."
    exit 1
fi

echo "OK: every native view with a fixture matches its capture and no design prohibition was found"
