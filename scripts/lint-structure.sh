#!/usr/bin/env bash
# lint-structure.sh — enforce file structure conventions
#
# Rules:
#   1. Cohesive modules. A .rs file that exports several top-level `pub`
#      items (struct/enum/fn/trait/type/const/static; `pub mod`, `pub use`,
#      `pub(crate)` and impl blocks do not count) is admitted only as a
#      facade internal: its parent declares it privately (`mod name;`, never
#      `pub mod name;`) and re-exports it (`pub use name::…`), it opens with
#      a `//!` module doc naming what the items decide together and why they
#      change together, and it has its companion `{name}.tests.rs`. A file
#      reached through a public module path still exports at most one item.
#      Each missing property is refused by name.
#   2. No `#[cfg(test)] mod tests` blocks — tests go in {name}.tests.rs files
#   3. Dependency direction: core ← detection ← agent ← orchestration
#      No reverse imports allowed (e.g., core must not import from detection)
#   4. Maximum function length (150 lines)
#   5. The desktop crate (crates/magistr-desktop) denies dead and unused code at
#      its root; no file under it may silence that with allow/expect
#
# Grandfathered files are in .magistr/lint-structure-allowlist.txt — a scoped
# baseline that is pruned as files are migrated or deleted, never regenerated
# to admit new violations. The `--generate` mode only reports what a fresh
# baseline would contain.
#
# Usage:
#   ./scripts/lint-structure.sh                    # check (default)
#   ./scripts/lint-structure.sh --generate         # list the files a fresh baseline would hold
#
# LINT_PROJECT_ROOT overrides the project root (integration tests run the
# script against a temp tree).

set -euo pipefail

PROJECT_ROOT="${LINT_PROJECT_ROOT:-$(cd "$(dirname "$0")/.." && pwd)}"
ALLOWLIST="$PROJECT_ROOT/.magistr/lint-structure-allowlist.txt"

is_allowed() {
    [[ -f "$ALLOWLIST" ]] && grep -qxF "$1" "$ALLOWLIST"
}

# The owned crates the cohesive-module and inline-test rules check. The
# imported subsystems (crates/magistr-workflow, crates/magistr-workspace) are
# ratcheted per file against their reviewed baselines by their own gates.
OWNED_SRC=(
    "$PROJECT_ROOT/crates/magistr-cli/src/"
    "$PROJECT_ROOT/crates/magistr-engine/src/"
    "$PROJECT_ROOT/crates/magistr-domain/src/"
    "$PROJECT_ROOT/crates/magistr-application/src/"
    "$PROJECT_ROOT/crates/magistr-workflow-contracts/src/"
)

owned_sources() {
    find "${OWNED_SRC[@]}" -name '*.rs' -not -name '*.tests.rs' 2>/dev/null
}

public_item_count() {
    grep -cE '^pub (struct|enum|fn|trait|type|const|static|async fn)' "$1" 2>/dev/null || true
}

# Rule 1: the cohesive-module rule. Prints one line per missing property of
# a file that exports several public items; prints nothing when the file is
# admitted. `$1` is the absolute path, `$2` the project-relative path.
cohesive_violations() {
    local f="$1" rel="$2" count
    count=$(public_item_count "$f")
    [[ "$count" -gt 1 ]] || return 0

    local dir name parent="" tests
    dir=$(dirname "$f")
    name=$(basename "$f" .rs)
    if [[ "$name" == "lib" || "$name" == "main" ]]; then
        echo "ERROR: $rel is a crate root exporting $count public items; a crate root is a facade, so move them behind a private module it re-exports"
        return 0
    fi
    if [[ "$name" == "mod" ]]; then
        name=$(basename "$dir")
        dir=$(dirname "$dir")
        tests="$(dirname "$f")/mod.tests.rs"
    else
        tests="${f%.rs}.tests.rs"
    fi
    # The parent facade: `dir/mod.rs`, the sibling `dir.rs`, or the crate root.
    for candidate in "$dir/mod.rs" "$dir.rs" "$dir/lib.rs" "$dir/main.rs"; do
        if [[ -f "$candidate" ]] && grep -qE "^[[:space:]]*(pub(\([a-z]+\))? )?mod ${name};" "$candidate"; then
            parent="$candidate"
            break
        fi
    done

    if [[ -z "$parent" ]]; then
        echo "ERROR: $rel exports $count public items but no parent facade declares \`mod ${name};\`; a cohesive module is a private module re-exported by its parent"
    elif grep -qE "^[[:space:]]*pub mod ${name};" "$parent"; then
        echo "ERROR: $rel exports $count public items through a public module path (\`pub mod ${name};\` in ${parent#"$PROJECT_ROOT/"}); declare it \`mod ${name};\` and re-export through the facade, or export one item"
    elif ! grep -qE "^[[:space:]]*pub(\([a-z]+\))? use (self::)?${name}::" "$parent"; then
        echo "ERROR: $rel exports $count public items but ${parent#"$PROJECT_ROOT/"} re-exports none of them (\`pub use ${name}::…\`); a private module nobody re-exports is dead surface, not a facade internal"
    fi
    if ! grep -m1 -E '\S' "$f" | grep -q '^//!'; then
        echo "ERROR: $rel exports $count public items without a \`//!\` module doc naming what they decide together and why they change together"
    fi
    if [[ ! -f "$tests" ]]; then
        echo "ERROR: $rel exports $count public items without its companion tests ${tests#"$PROJECT_ROOT/"}; behavior is pinned where it is owned"
    fi
}

# --- Generate mode -----------------------------------------------------------

if [[ "${1:-}" == "--generate" ]]; then
    echo "Files a fresh baseline would hold (the checked-in allowlist is not rewritten):"
    while IFS= read -r f; do
        rel="${f#"$PROJECT_ROOT/"}"
        if [[ -n "$(cohesive_violations "$f" "$rel")" ]] || grep -qE 'mod tests \{' "$f" 2>/dev/null; then
            echo "$rel"
        fi
    done < <(owned_sources) | sort -u
    exit 0
fi

# --- Check mode --------------------------------------------------------------

errors=0

# Native/reference workspace Rust uses numeric per-file ratchets for all
# structure rules, including inherited long functions. New files get the
# normal limits; growth in inherited violations fails. Synthetic lint test
# trees without this crate continue to exercise the legacy rules alone.
if [[ -d "$PROJECT_ROOT/crates/magistr-workspace" ]]; then
    if ! python3 -B "$PROJECT_ROOT/scripts/workspace-structure-gate.py" "$PROJECT_ROOT"; then
        errors=$((errors + 1))
    fi
fi

# Rule 1: Cohesive modules (see the header)
while IFS= read -r f; do
    rel="${f#"$PROJECT_ROOT/"}"
    is_allowed "$rel" && continue
    violations=$(cohesive_violations "$f" "$rel")
    if [[ -n "$violations" ]]; then
        echo "$violations"
        errors=$((errors + $(echo "$violations" | wc -l)))
    fi
done < <(owned_sources)

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
done < <(owned_sources)

# Rule 3: Dependency direction enforcement
#
# Domain layer ordering (lower layers must not import from higher layers):
#   core (0) ← detection (1) ← agent (2) ← orchestration (3)
#
# Re-exports in lib.rs (e.g., `use crate::error`) resolve to core;
# this rule checks direct domain module imports.
# Test files are excluded because test helpers may cross boundaries.

# Returns forbidden higher-layer modules for a given domain.
forbidden_for() {
    case "$1" in
        core)          echo "detection agent orchestration" ;;
        detection)     echo "agent orchestration" ;;
        agent)         echo "orchestration" ;;
        orchestration) echo "" ;;
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
done < <(find "$PROJECT_ROOT/crates/" -type d -name target -prune -o -path "$PROJECT_ROOT/crates/magistr-workspace" -prune -o -path "$PROJECT_ROOT/crates/magistr-workflow" -prune -o -name '*.rs' -not -name '*.tests.rs' -print 2>/dev/null)

# Rule 5: No lint silencing under the cockpit crates
#
# crates/magistr-cockpit and crates/magistr-desktop both carry
# #![deny(dead_code, unused)]; an allow or expect for either lint anywhere
# under them defeats the point (think:104). Dead code is deleted, not hidden.

for HYGIENE_CRATE in "$PROJECT_ROOT/crates/magistr-cockpit" "$PROJECT_ROOT/crates/magistr-desktop"; do
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
    echo "See .claude/rules/conventions.md (File Structure); the baseline $ALLOWLIST is pruned, not extended"
    exit 1
else
    echo "OK: structure lint passed"
    exit 0
fi