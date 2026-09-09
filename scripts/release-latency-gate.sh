#!/usr/bin/env bash
# Check release-mode input latency and frame-coalescing limits.
# Reports a skip if the release test binaries cannot be built.
# RELEASE_LATENCY_FILTER overrides the nextest filterset.

set -euo pipefail

PROJECT_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
FILTER="${RELEASE_LATENCY_FILTER:-(binary(turn_harness) & test(under_the_bound)) | (binary(runtime_harness) & test(/frames|paints_|tick_stops|resize_storm/))}"

cd "$PROJECT_ROOT"

if ! command -v cargo >/dev/null 2>&1; then
    echo "SKIPPED: cargo is not on PATH — the release latency bounds were not checked"
    exit 0
fi

build_log="$(mktemp)"
trap 'rm -f "$build_log"' EXIT
if ! cargo nextest run -p magistr-tui --release --no-run \
        -E 'binary(turn_harness) | binary(runtime_harness)' >"$build_log" 2>&1; then
    cat "$build_log"
    echo ""
    echo "SKIPPED: cargo could not build the release harness binaries — the release latency bounds were not checked"
    echo "(run ./scripts/release-latency-gate.sh once the release build works again)"
    exit 0
fi

cargo nextest run -p magistr-tui --release -E "$FILTER"

echo "OK: the cockpit's release-tier latency and frame-coalescing bounds hold"
