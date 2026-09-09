#!/usr/bin/env bash
# CI guard: verify magistr-engine has zero TUI dependencies.
#
# Checks the resolved dependency tree of magistr-engine for any
# ratatui, crossterm, or tachyonfx crates. Exits non-zero if found.

set -euo pipefail

FORBIDDEN=("ratatui" "crossterm" "tachyonfx")
CRATE="magistr-engine"

echo "Checking $CRATE dependency tree for forbidden TUI deps..."

TREE=$(cargo tree -p "$CRATE" --prefix none 2>&1)

FOUND=0
for dep in "${FORBIDDEN[@]}"; do
    if echo "$TREE" | grep -q "^${dep} "; then
        echo "ERROR: $CRATE depends on forbidden TUI crate: $dep"
        FOUND=1
    fi
done

if [ "$FOUND" -eq 0 ]; then
    echo "OK: $CRATE has no TUI dependencies."
    exit 0
else
    echo ""
    echo "Full dependency tree:"
    cargo tree -p "$CRATE"
    exit 1
fi
