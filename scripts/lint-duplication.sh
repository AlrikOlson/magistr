#!/usr/bin/env bash
# lint-duplication.sh — no code copied between the frozen legacy TUI and the
# rewrite (think:104).
#
# Two checks, both against the pair of trees LEGACY (default crates/magistr-cli/src/tui) and
# NEW (default crates/magistr-tui):
#   1. Identical files: a .rs file under NEW whose bytes equal a .rs file
#      under LEGACY is a copy, whatever it is named.
#   2. Clones: jscpd (Rust tokenizer, 50-token threshold) over both trees;
#      only clones with one side in each tree count — the legacy tree has
#      plenty of internal duplication and is frozen, not curated.
#
# Usage:
#   ./scripts/lint-duplication.sh
#
# Environment:
#   LINT_PROJECT_ROOT       project root (integration tests point it at a temp tree)
#   LINT_DUP_LEGACY         legacy tree, relative to the root (crates/magistr-cli/src/tui)
#   LINT_DUP_NEW            rewrite tree, relative to the root (crates/magistr-tui)
#   LINT_DUP_MIN_TOKENS     jscpd token threshold (50)
#   JSCPD                   the jscpd command (npx --yes jscpd)

set -euo pipefail

# Physical paths (pwd -P): jscpd reports canonical paths, and a temp root
# under a symlink (macOS /var → /private/var) must still match.
PROJECT_ROOT="$(cd "${LINT_PROJECT_ROOT:-$(dirname "$0")/..}" && pwd -P)"
LEGACY="$PROJECT_ROOT/${LINT_DUP_LEGACY:-crates/magistr-cli/src/tui}"
NEW="$PROJECT_ROOT/${LINT_DUP_NEW:-crates/magistr-tui}"
MIN_TOKENS="${LINT_DUP_MIN_TOKENS:-50}"
JSCPD="${JSCPD:-npx --yes jscpd}"

if [[ ! -d "$NEW" ]]; then
    echo "OK: no rewrite tree at ${NEW#"$PROJECT_ROOT/"} — nothing to compare"
    exit 0
fi
if [[ ! -d "$LEGACY" ]]; then
    echo "OK: no legacy tree at ${LEGACY#"$PROJECT_ROOT/"} — nothing to compare"
    exit 0
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

legacy_hashes="$(mktemp)"
trap 'rm -f "$legacy_hashes"' EXIT
while IFS= read -r f; do
    printf '%s %s\n' "$(hash_file "$f")" "${f#"$PROJECT_ROOT/"}" >> "$legacy_hashes"
done < <(find "$LEGACY" -type f -name '*.rs' 2>/dev/null)

while IFS= read -r f; do
    h="$(hash_file "$f")"
    twin="$(awk -v h="$h" '$1 == h { print $2; exit }' "$legacy_hashes")"
    if [[ -n "$twin" ]]; then
        echo "ERROR: ${f#"$PROJECT_ROOT/"} is byte-identical to $twin — the rewrite must not copy the legacy tree"
        errors=$((errors + 1))
    fi
done < <(find "$NEW" -type f -name '*.rs' 2>/dev/null)

# --- 2. Cross-tree clones (jscpd) ------------------------------------------------

report_dir="$(mktemp -d)"
trap 'rm -rf "$legacy_hashes" "$report_dir"' EXIT
# shellcheck disable=SC2086 # JSCPD is a command line, split on purpose
if ! jscpd_said="$($JSCPD --min-tokens "$MIN_TOKENS" --format rust --reporters json --output "$report_dir" \
        --silent --absolute --no-gitignore "$LEGACY" "$NEW" 2>&1)"; then
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

clones="$(python3 - "$report" "$LEGACY" "$NEW" "$PROJECT_ROOT" <<'PY'
import json, os, sys

report, legacy, new, root = sys.argv[1:5]
legacy, new, root = (os.path.realpath(p) for p in (legacy, new, root))

# Under the host's own separator: jscpd reports the paths it was given,
# and on Windows that is a `\` path however the shell spelled it — with
# the extended-length `\\?\` prefix, which realpath keeps.
def canonical(path):
    return os.path.realpath(path.removeprefix("\\\\?\\"))

def under(path, parent):
    return path.startswith(os.path.join(parent, ""))

def tree(path):
    path = canonical(path)
    if under(path, legacy):
        return "legacy"
    if under(path, new):
        return "new"
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
    if {tree(a["name"]), tree(b["name"])} == {"legacy", "new"}:
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
    echo "FAILED: $errors duplication violation(s) between ${LEGACY#"$PROJECT_ROOT/"} and ${NEW#"$PROJECT_ROOT/"}"
    echo "Rewrite it for the cockpit, or delete the legacy copy — never keep both."
    exit 1
fi
echo "OK: no code shared between ${LEGACY#"$PROJECT_ROOT/"} and ${NEW#"$PROJECT_ROOT/"}"
