#!/usr/bin/env bash
# Nextest setup script: give every test process of this run a fresh workflow
# store, so no test can read or upgrade the developer's own saved work. Tests
# that need their own store still set MAGISTR_WORKFLOW_DATA_DIR explicitly.
set -euo pipefail
store="$(mktemp -d "${TMPDIR:-/tmp}/magistr-test-store.XXXXXX")"
printf 'MAGISTR_WORKFLOW_DATA_DIR=%s\n' "$store" >> "$NEXTEST_ENV"
