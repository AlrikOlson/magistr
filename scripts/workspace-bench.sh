#!/usr/bin/env bash
# Record the native workspace perf runs (chunk arch-14c10b): the retained
# tier-0 / tier-1 / references-p95 / worker round-trip gates measured over
# generated scratch corpora against the release worker binary, written as
# workspace-bench/1 JSON under subsystems/workspace/bench/results/. Every
# profile runs even when an earlier one misses its gate; the exit status is
# the worst of them, and the JSON records the failure rather than hiding it.
#
# Usage: ./scripts/workspace-bench.sh [--loc N]

set -uo pipefail

PROJECT_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
RESULTS="$PROJECT_ROOT/subsystems/workspace/bench/results"
LOC="${2:-500000}"
if [[ "${1:-}" == "--loc" ]]; then
    LOC="$2"
fi
case "$LOC" in
    *000000) PROFILE="synthetic-$((LOC / 1000000))m" ;;
    *000) PROFILE="synthetic-$((LOC / 1000))k" ;;
    *) PROFILE="synthetic-$LOC" ;;
esac

cd "$PROJECT_ROOT"
mkdir -p "$RESULTS"
status=0

run_profile() {
    local profile="$1" out="$2"
    shift 2
    if ! cargo bench --locked -p magistr-cli --bench workspace_bench -- \
            --profile "$profile" --json "$out" "$@"; then
        status=1
    fi
}

run_profile worker "$RESULTS/worker-roundtrip.json"
run_profile index "$RESULTS/index-$PROFILE.json" --loc "$LOC"
run_profile refs "$RESULTS/refs-$PROFILE.json" --loc "$LOC"

if [[ "$status" -ne 0 ]]; then
    echo "FAIL: at least one perf gate missed — the JSON under $RESULTS records it"
    exit 1
fi
echo "OK: every perf gate held; results under $RESULTS"
