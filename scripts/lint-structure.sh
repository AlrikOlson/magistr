#!/usr/bin/env bash
# lint-structure.sh — enforce file structure conventions
#
# Rules:
#   1. Each .rs file has at most 1 top-level `pub` item
#      (excluding `pub mod`, `pub use`, `pub(crate)`, impl blocks)
#   2. No `#[cfg(test)] mod tests` blocks — tests go in {name}.tests.rs files
#   3. Dependency direction: core ← detection ← agent ← orchestration ← tui
#      No reverse imports allowed (e.g., core must not import from detection)
#   4. Maximum function length (150 lines)
#   5. The TUI crate (crates/magistr-tui) denies dead and unused code at
#      its root; no file under it may silence that with allow/expect
#
# Grandfathered files are in .magistr/lint-structure-allowlist.txt.
# New violations are blocked; existing ones are tracked for gradual migration.
#
# Usage:
#   ./scripts/lint-structure.sh                    # check (default)
#   ./scripts/lint-structure.sh --generate         # regenerate allowlist from current state
#
# LINT_PROJECT_ROOT overrides the project root (integration tests run the
# script against a temp tree).

set -euo pipefail

PROJECT_ROOT="${LINT_PROJECT_ROOT:-$(cd "$(dirname "$0")/.." && pwd)}"
ALLOWLIST="$PROJECT_ROOT/.magistr/lint-structure-allowlist.txt"

is_allowed() {
    [[ -f "$ALLOWLIST" ]] && grep -qxF "$1" "$ALLOWLIST"
}

# --- Generate mode -----------------------------------------------------------

if [[ "${1:-}" == "--generate" ]]; then
    echo "Generating allowlist..."
    : > "$ALLOWLIST"

    find "$PROJECT_ROOT/crates/magistr-cli/src/" "$PROJECT_ROOT/crates/magistr-engine/src/" "$PROJECT_ROOT/crates/magistr-domain/src/" "$PROJECT_ROOT/crates/magistr-application/src/" -name '*.rs' -not -name '*.tests.rs' 2>/dev/null | while read -r f; do
        rel="${f#"$PROJECT_ROOT/"}"
        count=$(grep -cE '^pub (struct|enum|fn|trait|type|const|static|async fn)' "$f" 2>/dev/null || true)
        if [ "$count" -gt 1 ]; then
            echo "$rel" >> "$ALLOWLIST"
        fi
        if grep -qE 'mod tests \{' "$f" 2>/dev/null; then
            echo "$rel" >> "$ALLOWLIST"
        fi
    done

    sort -u -o "$ALLOWLIST" "$ALLOWLIST"
    echo "Allowlist: $(wc -l < "$ALLOWLIST" | tr -d ' ') files"
    exit 0
fi

# --- Check mode --------------------------------------------------------------

errors=0

# Rule 1: One pub item per file
while IFS= read -r f; do
    rel="${f#"$PROJECT_ROOT/"}"
    count=$(grep -cE '^pub (struct|enum|fn|trait|type|const|static|async fn)' "$f" 2>/dev/null || true)
    if [[ "$count" -gt 1 ]] && ! is_allowed "$rel"; then
        echo "ERROR: $rel has $count top-level pub items (max 1)"
        errors=$((errors + 1))
    fi
done < <(find "$PROJECT_ROOT/crates/magistr-cli/src/" "$PROJECT_ROOT/crates/magistr-engine/src/" "$PROJECT_ROOT/crates/magistr-domain/src/" "$PROJECT_ROOT/crates/magistr-application/src/" -name '*.rs' -not -name '*.tests.rs' 2>/dev/null)

# Rule 2: No inline test modules
# A `#[cfg(test)] mod tests { ... }` block is an inline test module (violation).
# A `#[cfg(test)] ... mod tests;` (semicolon, no braces) is an external module
# declaration pointing to a .tests.rs file (allowed).
while IFS= read -r f; do
    rel="${f#"$PROJECT_ROOT/"}"
    if grep -qE '#\[cfg\(test\)\]' "$f" 2>/dev/null && ! is_allowed "$rel"; then
        # Check if any cfg(test) is followed by `mod tests {` (inline block)
        # vs `mod tests;` (external declaration, which is fine).
        if grep -qE 'mod tests \{' "$f" 2>/dev/null; then
            echo "ERROR: $rel has inline #[cfg(test)] mod tests — move tests to ${rel%.rs}.tests.rs"
            errors=$((errors + 1))
        fi
    fi
done < <(find "$PROJECT_ROOT/crates/magistr-cli/src/" "$PROJECT_ROOT/crates/magistr-engine/src/" "$PROJECT_ROOT/crates/magistr-domain/src/" "$PROJECT_ROOT/crates/magistr-application/src/" -name '*.rs' -not -name '*.tests.rs' 2>/dev/null)

# Rule 3: Dependency direction enforcement
#
# Domain layer ordering (lower layers must not import from higher layers):
#   core (0) ← detection (1) ← agent (2) ← orchestration (3) ← tui (4)
#
# Backward-compat re-exports in lib.rs (e.g., `use crate::error`) are allowed
# because they resolve to core — we only check direct domain module imports.
# Test files are excluded because test helpers may cross boundaries.

# Returns forbidden higher-layer modules for a given domain.
forbidden_for() {
    case "$1" in
        core)          echo "detection agent orchestration tui" ;;
        detection)     echo "agent orchestration tui" ;;
        agent)         echo "orchestration tui" ;;
        orchestration) echo "tui" ;;
    esac
}

DEP_ALLOWLIST="$PROJECT_ROOT/.magistr/lint-dep-direction-allowlist.txt"

is_dep_allowed() {
    [[ -f "$DEP_ALLOWLIST" ]] && grep -qxF "$1" "$DEP_ALLOWLIST"
}

for domain in core detection agent orchestration; do
    forbidden=$(forbidden_for "$domain")
    domain_dir="$PROJECT_ROOT/crates/magistr-cli/src/$domain"
    [[ -d "$domain_dir" ]] || continue

    for target in $forbidden; do
        # Search for `use crate::<target>::` in non-test source files
        while IFS= read -r f; do
            rel="${f#"$PROJECT_ROOT/"}"
            matches=$(grep -nE "use crate::${target}::" "$f" 2>/dev/null || true)
            if [[ -n "$matches" ]]; then
                while IFS= read -r match; do
                    if ! is_dep_allowed "$rel"; then
                        echo "ERROR: $rel — forbidden import from $domain → $target: $match"
                        errors=$((errors + 1))
                    fi
                done <<< "$matches"
            fi
        done < <(find "$domain_dir" -name '*.rs' -not -name '*.tests.rs' 2>/dev/null)
    done
done

# Rule 4: Maximum function/closure length (150 lines)
#
# Scans for function definitions and counts their body lines. Functions over
# 150 lines violate SRP and should be decomposed.

MAX_FN_LINES=150
FN_LENGTH_ALLOWLIST="$PROJECT_ROOT/.magistr/lint-fn-length-allowlist.txt"

is_fn_allowed() {
    [[ -f "$FN_LENGTH_ALLOWLIST" ]] && grep -qxF "$1" "$FN_LENGTH_ALLOWLIST"
}

while IFS= read -r f; do
    rel="${f#"$PROJECT_ROOT/"}"
    # Use awk to count lines between function start and matching close brace.
    # Approximation: counts `pub fn` / `fn` blocks by brace depth.
    while IFS=: read -r line_num fn_name fn_lines; do
        if [[ "$fn_lines" -gt "$MAX_FN_LINES" ]] && ! is_fn_allowed "$rel:$fn_name"; then
            echo "ERROR: $rel:$line_num — function '$fn_name' is $fn_lines lines (max $MAX_FN_LINES)"
            errors=$((errors + 1))
        fi
    done < <(awk '
        /^[[:space:]]*(pub(\(crate\))? )?(async )?fn [a-z_]/ {
            name = $0;
            sub(/.*fn /, "", name);
            sub(/[(<].*/, "", name);
            start = NR;
            depth = 0;
            do {
                getline;
                gsub(/[^{}]/, "");
                depth += gsub(/{/, "{");
                depth -= gsub(/}/, "}");
            } while (depth > 0 && NR < start + 500);
            lines = NR - start;
            if (lines > '"$MAX_FN_LINES"')
                print start ":" name ":" lines;
        }
    ' "$f" 2>/dev/null)
done < <(find "$PROJECT_ROOT/crates/" -name '*.rs' -not -name '*.tests.rs' 2>/dev/null)

# Rule 5: No lint silencing under the cockpit crates
#
# crates/magistr-cockpit and crates/magistr-tui both carry
# #![deny(dead_code, unused)]; an allow or expect for either lint anywhere
# under them defeats the point (think:104). Dead code is deleted, not hidden.

for HYGIENE_CRATE in "$PROJECT_ROOT/crates/magistr-cockpit" "$PROJECT_ROOT/crates/magistr-tui"; do
    [[ -d "$HYGIENE_CRATE" ]] || continue
    while IFS= read -r f; do
        rel="${f#"$PROJECT_ROOT/"}"
        matches=$(grep -nE '#!?\[(allow|expect)\([^]]*(dead_code|unused)' "$f" 2>/dev/null || true)
        if [[ -n "$matches" ]]; then
            while IFS= read -r match; do
                echo "ERROR: $rel — the crate denies dead/unused code; delete it instead of silencing the lint: $match"
                errors=$((errors + 1))
            done <<< "$matches"
        fi
    done < <(find "$HYGIENE_CRATE" -name '*.rs' -not -path '*/target/*' 2>/dev/null)
done

if [[ "$errors" -gt 0 ]]; then
    echo ""
    echo "FAILED: $errors structure violation(s)"
    echo "If these are intentional, add them to $ALLOWLIST"
    exit 1
else
    echo "OK: structure lint passed"
    exit 0
fi
