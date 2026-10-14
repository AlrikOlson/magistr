#!/bin/bash
# Double-click in Finder to build and open Magistr.
export PATH="/opt/homebrew/bin:/usr/local/bin:$PATH"
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
if ! command -v node >/dev/null 2>&1; then
    echo "Install Node.js 22 or later from https://nodejs.org, then try again."
    result=1
else
    node "$SCRIPT_DIR/scripts/desktop.mjs" open "$@"
    result=$?
fi
if [ "$result" -ne 0 ] && [ -t 0 ]; then
    read -r -p "Press Return to close this window. "
fi
exit "$result"
