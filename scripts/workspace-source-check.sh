#!/usr/bin/env bash
# Inspect source provenance and Cargo composition without opening a workspace owner.
set -euo pipefail

WORKSPACE_ROOT="${WORKSPACE_SOURCE_PROJECT_ROOT:-$(cd "$(dirname "$0")/.." && pwd)}"
python3 -B "$WORKSPACE_ROOT/scripts/workspace-source-check.py" "$WORKSPACE_ROOT"
python3 -B "$WORKSPACE_ROOT/scripts/workspace-structure-check.py" "$WORKSPACE_ROOT"
python3 -B "$WORKSPACE_ROOT/scripts/workspace_source_check_tests.py"
