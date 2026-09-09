#!/usr/bin/env bash
# Reject unknown nextest configuration keys and profile filters with no tests.
# Reports a skip if Cargo cannot build the test binaries.

set -euo pipefail

PROJECT_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
CONFIG="$PROJECT_ROOT/.config/nextest.toml"
SCOPE=(--workspace)

cd "$PROJECT_ROOT"

if ! command -v cargo >/dev/null 2>&1; then
    echo "SKIPPED: cargo is not on PATH — .config/nextest.toml was not checked"
    exit 0
fi

if [ ! -f "$CONFIG" ]; then
    echo "SKIPPED: $CONFIG does not exist"
    exit 0
fi

log="$(mktemp)"
trap 'rm -f "$log"' EXIT

# One list over the default profile: it both proves the binaries build and
# gives nextest the chance to report every unknown key in the file at once.
if ! cargo nextest list "${SCOPE[@]}" >"$log" 2>&1; then
    cat "$log"
    echo ""
    echo "SKIPPED: cargo could not list the workspace's tests — .config/nextest.toml was not checked"
    echo "(run ./scripts/nextest-config-gate.sh once the build works again)"
    exit 0
fi

if grep -q "unknown configuration key" "$log"; then
    echo "FAILED: nextest does not recognise every key in .config/nextest.toml"
    echo ""
    grep -E "unknown configuration key|^  - " "$log"
    echo ""
    echo "Remove the key or replace it with the current spelling — see"
    echo "https://nexte.st/docs/configuration/reference/ for the profile keys."
    exit 1
fi

# Every profile that declares a default-filter must select something. Section
# names carrying a dot ([profile.default.junit]) are sub-tables, and a
# default-filter never appears in one.
filtered=$(awk '
    /^\[profile\./ { name = $0; sub(/^\[profile\./, "", name); sub(/\].*$/, "", name) }
    /^[[:space:]]*default-filter[[:space:]]*=/ { print name }
' "$CONFIG")

status=0
for profile in $filtered; do
    if ! cargo nextest list "${SCOPE[@]}" -P "$profile" --run-ignored all >"$log" 2>&1; then
        cat "$log"
        echo "FAILED: profile '$profile' could not be listed"
        status=1
        continue
    fi
    count=$(awk 'NF && $0 !~ /^ / { n++ } END { print n + 0 }' "$log")
    if [ "$count" -eq 0 ]; then
        echo "FAILED: profile '$profile' has a default-filter that selects no test"
        echo "  $(awk -v p="$profile" '
            /^\[profile\./ { name = $0; sub(/^\[profile\./, "", name); sub(/\].*$/, "", name) }
            name == p && /^[[:space:]]*default-filter[[:space:]]*=/ { print; exit }
        ' "$CONFIG")"
        echo "  Either the tests it names are gone, or the filterset no longer matches them."
        status=1
    else
        echo "  profile '$profile' selects $count test(s)"
    fi
done

if [ "$status" -ne 0 ]; then
    exit 1
fi

echo "OK: nextest knows every key in .config/nextest.toml and every profile filter selects tests"
