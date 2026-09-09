#!/usr/bin/env bash
INPUT=$(cat)
echo "$INPUT" > /tmp/hook-debug-output.json
exit 0
