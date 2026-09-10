#!/usr/bin/env bash
# Verify first-party workflow ownership without opening a workflow store/server.
set -euo pipefail

WORKFLOW_ROOT="${WORKFLOW_SOURCE_PROJECT_ROOT:-$(cd "$(dirname "$0")/.." && pwd)}"
python3 "$WORKFLOW_ROOT/scripts/workflow-source-gate.py" "$WORKFLOW_ROOT"
python3 "$WORKFLOW_ROOT/scripts/workflow-structure-gate.py" "$WORKFLOW_ROOT"
