#!/usr/bin/env bash
# Enforce the tracked file/line limits in .magistr/legacy-gui-ratchet.json.
# Binary files and untracked build outputs do not contribute to line counts.
# --update records a smaller baseline; increases are rejected.
# RATCHET_PROJECT_ROOT selects a different project root for integration tests.

set -euo pipefail

PROJECT_ROOT="${RATCHET_PROJECT_ROOT:-$(cd "$(dirname "$0")/.." && pwd)}"
RATCHET="$PROJECT_ROOT/.magistr/legacy-gui-ratchet.json"

if [[ ! -f "$RATCHET" ]]; then
    echo "legacy-gui: no frozen-tree declaration at .magistr/legacy-gui-ratchet.json; nothing to gate"
    exit 0
fi

# --- Declaration ------------------------------------------------------------
# Flat JSON: {"path": …, "files": N, "loc": M, "unlock": [...], "replacement": …}
# Parsed with sed so the gate has no jq dependency.

ratchet_number() {
    sed -nE "s/.*\"$1\"[[:space:]]*:[[:space:]]*([0-9]+).*/\1/p" "$RATCHET" | head -n 1
}

ratchet_string() {
    tr -d '\n' < "$RATCHET" | sed -nE "s/.*\"$1\"[[:space:]]*:[[:space:]]*\"([^\"]*)\".*/\1/p"
}

ratchet_unlock() {
    tr -d '\n' < "$RATCHET" | sed -nE 's/.*"unlock"[[:space:]]*:[[:space:]]*(\[[^]]*\]).*/\1/p'
}

FROZEN_REL="$(ratchet_string path)"
if [[ -z "$FROZEN_REL" ]]; then
    echo "ERROR: legacy-gui — the declaration names no \"path\""
    exit 1
fi

if ! git -C "$PROJECT_ROOT" rev-parse --is-inside-work-tree >/dev/null 2>&1; then
    echo "ERROR: legacy-gui — $PROJECT_ROOT declares $FROZEN_REL frozen but is not inside a git work tree; the ratchet counts tracked files and cannot measure this tree"
    exit 1
fi

# --- Measurement ------------------------------------------------------------
# Every tracked file under the frozen tree — a new file of any kind is growth.

count_files() {
    git -C "$PROJECT_ROOT" ls-files -- "$1" | grep -c . || true
}

# Lines across every tracked TEXT file (-I skips binaries). `^` matches every
# line, so the per-file counts sum to the tree's line count; a file git does
# not track, or one that is empty, contributes nothing.
count_loc() {
    local counts status=0
    counts="$(git -C "$PROJECT_ROOT" grep -I -c '^' -- "$1")" || status=$?
    if [[ "$status" -gt 1 ]]; then
        return "$status"
    fi
    printf '%s\n' "$counts" | awk -F: '{ total += $NF } END { print total + 0 }'
}

BASE_FILES="$(ratchet_number files)"
BASE_LOC="$(ratchet_number loc)"
CUR_FILES="$(count_files "$FROZEN_REL")"
CUR_LOC="$(count_loc "$FROZEN_REL")"

# --- Update mode ------------------------------------------------------------

if [[ "${1:-}" == "--update" ]]; then
    if [[ -n "$BASE_FILES" && "$CUR_FILES" -gt "$BASE_FILES" ]] \
        || [[ -n "$BASE_LOC" && "$CUR_LOC" -gt "$BASE_LOC" ]]; then
        echo "ERROR: legacy-gui — the ratchet only moves down; $FROZEN_REL grew (files $BASE_FILES → $CUR_FILES, loc $BASE_LOC → $CUR_LOC). Delete, don't add."
        exit 1
    fi
    unlock="$(ratchet_unlock)"
    [[ -n "$unlock" ]] || unlock='["legacy-gui", "delete"]'
    replacement="$(ratchet_string replacement)"
    {
        printf '{\n  "path": "%s",\n  "files": %s,\n  "loc": %s,\n  "unlock": %s' \
            "$FROZEN_REL" "$CUR_FILES" "$CUR_LOC" "$unlock"
        [[ -n "$replacement" ]] && printf ',\n  "replacement": "%s"' "$replacement"
        printf '\n}\n'
    } > "$RATCHET"
    echo "legacy-gui: baseline recorded — $FROZEN_REL files=$CUR_FILES loc=$CUR_LOC"
    exit 0
fi

# --- Check mode -------------------------------------------------------------

errors=0
if [[ -n "$BASE_FILES" && "$CUR_FILES" -gt "$BASE_FILES" ]]; then
    echo "ERROR: legacy-gui — $FROZEN_REL tracks $CUR_FILES files, baseline $BASE_FILES (frozen tree: delete, don't add; see $FROZEN_REL/README.md)"
    errors=$((errors + 1))
fi
if [[ -n "$BASE_LOC" && "$CUR_LOC" -gt "$BASE_LOC" ]]; then
    echo "ERROR: legacy-gui — $FROZEN_REL holds $CUR_LOC lines, baseline $BASE_LOC (frozen tree: delete, don't add; see $FROZEN_REL/README.md)"
    errors=$((errors + 1))
fi

echo "legacy-gui: $FROZEN_REL files=$CUR_FILES/$BASE_FILES loc=$CUR_LOC/$BASE_LOC"

if [[ "$errors" -gt 0 ]]; then
    echo ""
    echo "FAILED: the frozen GUI source exceeds its recorded baseline."
    exit 1
fi

echo "OK"
