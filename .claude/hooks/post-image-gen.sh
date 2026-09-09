#!/usr/bin/env bash
# PostToolUse hook for mcp__mcp-image__generate_image
#
# Automatically runs the green screen extraction pipeline on every
# AI-generated image, producing a transparent PNG alongside the original.
#
# The hook reads tool_response from stdin, extracts the file URI,
# runs extract-foreground.py, and returns the transparent PNG path
# back to Claude via updatedMCPToolOutput.
set -euo pipefail

INPUT=$(cat)

# Extract the generated image path from tool_response
# tool_response is an array: [{"type":"text","text":"{\"type\":\"resource\",\"resource\":{\"uri\":\"file:///...\"},...}"}]
# We need to parse the inner JSON string from .tool_response[0].text
INNER_JSON=$(echo "$INPUT" | jq -r '.tool_response[0].text // empty' 2>/dev/null)
IMAGE_URI=$(echo "$INNER_JSON" | jq -r '.resource.uri // empty' 2>/dev/null)

if [[ -z "$IMAGE_URI" ]]; then
    # Not a resource response, or missing URI — skip silently
    exit 0
fi

# Convert file:// URI to local path
IMAGE_PATH="${IMAGE_URI#file://}"

if [[ ! -f "$IMAGE_PATH" ]]; then
    exit 0
fi

# Determine output path: same name with -transparent.png suffix
DIR=$(dirname "$IMAGE_PATH")
BASE=$(basename "$IMAGE_PATH")
# Strip any extension
STEM="${BASE%.*}"
OUTPUT_PATH="${DIR}/${STEM}-transparent.png"

PROJECT_DIR="${CLAUDE_PROJECT_DIR:-$(cd "$(dirname "$0")/../.." && pwd)}"
SCRIPT="${PROJECT_DIR}/scripts/extract-foreground.py"

if [[ ! -f "$SCRIPT" ]]; then
    echo "extract-foreground.py not found at $SCRIPT" >&2
    exit 0
fi

# Detect best python environment:
# 1. CorridorKey venv (has BiRefNet + torch — Tier 1)
# 2. uv run with numpy+pillow (Tier 3 fallback)
CK_VENV="$HOME/Code/CorridorKey/.venv/bin/python"

if [[ -x "$CK_VENV" ]]; then
    PYTHON="$CK_VENV"
    TIER_FLAG="--tier corridorkey"
else
    # Use uv run for zero-dep fallback
    if command -v uv &>/dev/null; then
        PYTHON="uv run --with numpy --with pillow python3"
        TIER_FLAG="--tier chroma"
    else
        echo "No suitable Python environment found for image extraction" >&2
        exit 0
    fi
fi

# Run extraction (suppress progress spam, capture errors)
EXTRACT_OUTPUT=$($PYTHON "$SCRIPT" "$IMAGE_PATH" "$OUTPUT_PATH" $TIER_FLAG 2>&1) || {
    echo "Image extraction failed: $EXTRACT_OUTPUT" >&2
    exit 0  # Don't block Claude on extraction failure
}

# Return the transparent PNG path back to Claude
# updatedMCPToolOutput replaces the original tool response
cat <<EOF
{
  "hookSpecificOutput": {
    "hookEventName": "PostToolUse",
    "additionalContext": "Auto-extracted transparent PNG: ${OUTPUT_PATH}\nOriginal: ${IMAGE_PATH}\nExtraction tier: ${TIER_FLAG#--tier }"
  }
}
EOF
