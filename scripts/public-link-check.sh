#!/usr/bin/env bash
# Check the public endpoints named by the README and website.
# A failed probe is an external release gate until the repository/DNS owner fixes it.
set -uo pipefail

urls=(
  "https://github.com/AlrikOlson/magistr"
  "https://github.com/AlrikOlson/magistr/releases"
  "https://github.com/AlrikOlson/magistr/issues"
  "https://magistr.dev"
)

failures=0
for url in "${urls[@]}"; do
  if status="$(curl -L -sS --max-time 20 -o /dev/null -w '%{http_code}' "$url" 2>/dev/null)"; then
    :
  else
    status="000"
  fi
  if [[ "$status" =~ ^2|^3 ]]; then
    printf 'PASS %s (%s)\n' "$url" "$status"
  else
    printf 'EXTERNAL GATE %s (%s)\n' "$url" "$status"
    failures=$((failures + 1))
  fi
done

if (( failures > 0 )); then
  printf '\nPUBLIC RELEASE BLOCKED: %d endpoint(s) are not anonymously reachable.\n' "$failures"
  printf 'Owner: repository maintainer / hosting operator. Follow-up: make the GitHub repository public and publish magistr.dev, then rerun this check.\n'
  if [[ "${PUBLIC_RELEASE_STRICT:-0}" == "1" ]]; then
    exit 2
  fi
fi
