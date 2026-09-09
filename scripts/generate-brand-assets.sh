#!/usr/bin/env bash
# Generate all brand asset sizes from source images in .magistr/generated-images/
# Requires: sips (macOS built-in) or ImageMagick (convert)
#
# Usage: ./scripts/generate-brand-assets.sh
#
# Source images (in .magistr/generated-images/):
#   magistr-icon-512       — M glyph app icon (used for favicons)
#   magistr-og-card        — OG social card
#   magistr-github-social  — GitHub social preview

set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
SRC="$ROOT/.magistr/generated-images"
WWW="$ROOT/apps/website/public"

# ── Check source images exist ────────────────────────────────────────────────

check_source() {
    if [[ ! -f "$SRC/$1" ]]; then
        echo "ERROR: Missing source image: $SRC/$1" >&2
        echo "Generate it first with mcp-image generate_image" >&2
        exit 1
    fi
}

check_source "magistr-icon-512"
check_source "magistr-og-card"
check_source "magistr-github-social"

# ── Resize helper (prefers sips on macOS, falls back to ImageMagick) ─────────

resize_png() {
    local src="$1" size="$2" dest="$3"
    if command -v sips &>/dev/null; then
        sips -s format png -Z "$size" "$src" --out "$dest" >/dev/null 2>&1
    elif command -v convert &>/dev/null; then
        convert "$src" -resize "${size}x${size}" -format png "$dest"
    else
        echo "ERROR: Need sips (macOS) or ImageMagick (convert) for resizing" >&2
        exit 1
    fi
    echo "  ✓ $dest (${size}x${size})"
}

copy_png() {
    local src="$1" dest="$2"
    if command -v sips &>/dev/null; then
        sips -s format png "$src" --out "$dest" >/dev/null 2>&1
    elif command -v convert &>/dev/null; then
        convert "$src" -format png "$dest"
    else
        cp "$src" "$dest"
    fi
    echo "  ✓ $dest"
}

# ── Generate favicon set ────────────────────────────────────────────────────

echo "Generating favicon set from magistr-icon-512..."
resize_png "$SRC/magistr-icon-512" 512 "$WWW/icon-512.png"
resize_png "$SRC/magistr-icon-512" 192 "$WWW/icon-192.png"
resize_png "$SRC/magistr-icon-512" 180 "$WWW/apple-touch-icon.png"
resize_png "$SRC/magistr-icon-512" 32  "$WWW/favicon-32.png"
resize_png "$SRC/magistr-icon-512" 16  "$WWW/favicon-16.png"

# ── Deploy OG card ──────────────────────────────────────────────────────────

echo "Deploying OG social card..."
copy_png "$SRC/magistr-og-card" "$WWW/og-image.png"

# ── Deploy GitHub social preview ────────────────────────────────────────────

echo "Deploying GitHub social preview..."
copy_png "$SRC/magistr-github-social" "$WWW/github-social.png"

echo ""
echo "Done! All brand assets deployed."
echo ""
echo "Files updated:"
echo "  apps/website/public/icon-512.png"
echo "  apps/website/public/icon-192.png"
echo "  apps/website/public/apple-touch-icon.png"
echo "  apps/website/public/favicon-32.png"
echo "  apps/website/public/favicon-16.png"
echo "  apps/website/public/og-image.png"
echo "  apps/website/public/github-social.png"

