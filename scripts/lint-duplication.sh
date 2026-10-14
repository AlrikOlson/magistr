#!/usr/bin/env bash
# lint-duplication.sh — no code copied between the application owner and the
# desktop (think:104).
#
# Two checks, both against the pair of trees OWNER (default crates/magistr-cli/src/application) and
# SURFACE (default crates/magistr-desktop/src):
#   1. Identical files: a .rs file under SURFACE whose bytes equal a .rs file
#      under OWNER is a copy, whatever it is named.
#   2. Clones: jscpd (Rust tokenizer, 50-token threshold) over both trees;
#      only clones with one side in each tree count — the owner tree has
#      its internal duplication is checked separately.
#
# Usage:
#   ./scripts/lint-duplication.sh
#
# Environment:
#   LINT_PROJECT_ROOT       project root (integration tests point it at a temp tree)
#   LINT_DUP_OWNER         owner tree, relative to the root (crates/magistr-cli/src/application)
#   LINT_DUP_SURFACE            desktop tree, relative to the root (crates/magistr-desktop/src)
#   LINT_DUP_MIN_TOKENS     jscpd token threshold (50)
#   JSCPD                   the jscpd command (npx --yes jscpd)

set -euo pipefail

# Physical paths (pwd -P): jscpd reports canonical paths, and a temp root
# under a symlink (macOS /var → /private/var) must still match.
PROJECT_ROOT="$(cd "${LINT_PROJECT_ROOT:-$(dirname "$0")/..}" && pwd -P)"
OWNER="$PROJECT_ROOT/${LINT_DUP_OWNER:-crates/magistr-cli/src/application}"
SURFACE="$PROJECT_ROOT/${LINT_DUP_SURFACE:-crates/magistr-desktop/src}"
MIN_TOKENS="${LINT_DUP_MIN_TOKENS:-50}"
JSCPD="${JSCPD:-npx --yes jscpd}"

if [[ ! -d "$SURFACE" ]]; then
    echo "ERROR: missing desktop tree at ${SURFACE#"$PROJECT_ROOT/"} "
    exit 1
fi
if [[ ! -d "$OWNER" ]]; then
    echo "ERROR: missing owner tree at ${OWNER#"$PROJECT_ROOT/"} "
    exit 1
fi

hash_file() {
    if command -v sha256sum >/dev/null 2>&1; then
        sha256sum "$1" | cut -d' ' -f1
    else
        shasum -a 256 "$1" | cut -d' ' -f1
    fi
}

errors=0

# --- 1. Identical files --------------------------------------------------------

owner_hashes="$(mktemp)"
trap 'rm -f "$owner_hashes"' EXIT
while IFS= read -r f; do
    printf '%s %s\n' "$(hash_file "$f")" "${f#"$PROJECT_ROOT/"}" >> "$owner_hashes"
done < <(find "$OWNER" -type f -name '*.rs' 2>/dev/null)

while IFS= read -r f; do
    h="$(hash_file "$f")"
    twin="$(awk -v h="$h" '$1 == h { print $2; exit }' "$owner_hashes")"
    if [[ -n "$twin" ]]; then
        echo "ERROR: ${f#"$PROJECT_ROOT/"} is byte-identical to $twin — the desktop must not copy the owner tree"
        errors=$((errors + 1))
    fi
done < <(find "$SURFACE" -type f -name '*.rs' 2>/dev/null)

# --- 2. Cross-tree clones (jscpd) ------------------------------------------------

report_dir="$(mktemp -d)"
trap 'rm -rf "$owner_hashes" "$report_dir"' EXIT
# shellcheck disable=SC2086 # JSCPD is a command line, split on purpose
if ! jscpd_said="$($JSCPD --min-tokens "$MIN_TOKENS" --format rust --reporters json --output "$report_dir" \
        --silent --absolute --no-gitignore "$OWNER" "$SURFACE" 2>&1)"; then
    echo "ERROR: jscpd did not run (command: $JSCPD) — install node or set JSCPD"
    printf '%s
' "$jscpd_said" | tail -n 20
    exit 1
fi
report="$report_dir/jscpd-report.json"
if [[ ! -f "$report" ]]; then
    echo "ERROR: jscpd wrote no report to $report_dir"
    exit 1
fi

clones="$(python3 - "$report" "$OWNER" "$SURFACE" "$PROJECT_ROOT" <<'PY'
import json, os, sys

report, owner, surface, root = sys.argv[1:5]
owner, surface, root = (os.path.realpath(p) for p in (owner, surface, root))

# Under the host's own separator: jscpd reports the paths it was given,
# and on Windows that is a `\` path however the shell spelled it — with
# the extended-length `\\?\` prefix, which realpath keeps.
def canonical(path):
    return os.path.realpath(path.removeprefix("\\\\?\\"))

def under(path, parent):
    return path.startswith(os.path.join(parent, ""))

def tree(path):
    path = canonical(path)
    if under(path, owner):
        return "owner"
    if under(path, surface):
        return "surface"
    return None

def rel(path):
    path = canonical(path)
    if under(path, root):
        path = path[len(root) + 1:]
    return path.replace(os.sep, "/")

with open(report, encoding="utf-8") as fh:
    data = json.load(fh)
for dup in data.get("duplicates", []):
    a, b = dup["firstFile"], dup["secondFile"]
    if {tree(a["name"]), tree(b["name"])} == {"owner", "surface"}:
        print(
            f"{rel(a['name'])}:{a['start']}-{a['end']} duplicates "
            f"{rel(b['name'])}:{b['start']}-{b['end']} ({dup.get('tokens', '?')} tokens)"
        )
PY
)"
if [[ -n "$clones" ]]; then
    while IFS= read -r line; do
        echo "ERROR: clone — $line"
        errors=$((errors + 1))
    done <<< "$clones"
fi

if [[ "$errors" -gt 0 ]]; then
    echo ""
    echo "FAILED: $errors duplication violation(s) between ${OWNER#"$PROJECT_ROOT/"} and ${SURFACE#"$PROJECT_ROOT/"}"
    echo "Move the shared behavior to its application owner; the desktop calls it."
    exit 1
fi
echo "OK: no code shared between ${OWNER#"$PROJECT_ROOT/"} and ${SURFACE#"$PROJECT_ROOT/"}"