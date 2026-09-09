#!/usr/bin/env python3
"""Trace a PNG logo into a clean SVG using potrace.

Usage:
    uv run --with pillow scripts/png-to-svg.py <input.png> <output.svg> [--viewbox WxH] [--fill COLOR]

The script:
1. Loads the PNG with Pillow
2. Converts to grayscale, thresholds to 1-bit bitmap
3. Writes a temporary PBM file
4. Runs potrace to produce an SVG
5. Optionally wraps in a custom viewBox and fill color
"""
import argparse
import subprocess
import sys
import tempfile
from pathlib import Path


def main():
    parser = argparse.ArgumentParser(description="Trace PNG to SVG via potrace")
    parser.add_argument("input", help="Input PNG file")
    parser.add_argument("output", help="Output SVG file")
    parser.add_argument("--viewbox", default=None, help="Custom viewBox as WxH (e.g. 32x32)")
    parser.add_argument("--fill", default="#7c3aed", help="Fill color for the traced path")
    parser.add_argument("--bg", default=None, help="Background rect color (e.g. #1a1b26)")
    parser.add_argument("--bg-rx", default="0", help="Background rect corner radius")
    parser.add_argument("--threshold", type=int, default=128, help="Grayscale threshold (0-255)")
    parser.add_argument("--invert", action="store_true", help="Invert bitmap (trace dark areas)")
    args = parser.parse_args()

    try:
        from PIL import Image
    except ImportError:
        print("Error: Pillow required. Run: uv run --with pillow scripts/png-to-svg.py ...", file=sys.stderr)
        sys.exit(1)

    img = Image.open(args.input)

    # Use alpha channel if present, otherwise use luminance
    if img.mode == "RGBA":
        # Extract alpha — trace the visible (non-transparent) region
        alpha = img.split()[3]
        bw = alpha.point(lambda p: 0 if p > args.threshold else 255, "1")
    else:
        gray = img.convert("L")
        bw = gray.point(lambda p: 0 if p > args.threshold else 255, "1")

    if args.invert:
        bw = bw.point(lambda p: 255 - p)

    with tempfile.NamedTemporaryFile(suffix=".pbm", delete=False) as tmp:
        pbm_path = tmp.name
        bw.save(pbm_path)

    # Run potrace: PBM -> SVG
    with tempfile.NamedTemporaryFile(suffix=".svg", delete=False) as tmp:
        svg_tmp = tmp.name

    subprocess.run(
        ["potrace", pbm_path, "-s", "-o", svg_tmp, "--flat", "--turdsize", "10"],
        check=True,
    )

    # Read the raw potrace SVG and extract the path data
    svg_content = Path(svg_tmp).read_text()

    # Parse dimensions from potrace output and extract path
    import re

    # Extract all path d= attributes
    paths = re.findall(r'd="([^"]+)"', svg_content)

    if not paths:
        print("Error: no paths found in potrace output", file=sys.stderr)
        sys.exit(1)

    # Get original image dimensions for coordinate mapping
    w, h = img.size

    # Determine output viewBox
    if args.viewbox:
        vw, vh = args.viewbox.split("x")
        viewbox = f"0 0 {vw} {vh}"
        # potrace uses the image dimensions, so we need to scale
        scale_x = float(vw) / w
        scale_y = float(vh) / h
    else:
        viewbox = f"0 0 {w} {h}"
        scale_x = 1.0
        scale_y = 1.0

    vw_val = float(viewbox.split()[2])
    vh_val = float(viewbox.split()[3])

    # Build SVG
    lines = [
        f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="{viewbox}">',
    ]

    if args.bg:
        lines.append(f'  <rect width="{vw_val}" height="{vh_val}" rx="{args.bg_rx}" fill="{args.bg}"/>')

    # potrace outputs coordinates in the image's pixel space but Y-flipped
    # We need to apply a transform to flip Y and scale
    transform_parts = []
    if scale_x != 1.0 or scale_y != 1.0:
        transform_parts.append(f"scale({scale_x},{scale_y})")

    # Potrace SVGs have their own transform; let's just use the raw potrace SVG
    # but extract and re-wrap the paths with our styling
    # Actually, potrace already handles the coordinate system — let's just
    # re-wrap with our viewBox and colors

    # Re-read potrace SVG more carefully
    # potrace outputs: <svg width="Wpt" height="Hpt" viewBox="0 0 W H">
    potrace_vb = re.search(r'viewBox="([^"]+)"', svg_content)
    potrace_w = re.search(r'width="([^"]+)"', svg_content)
    potrace_h = re.search(r'height="([^"]+)"', svg_content)

    # Extract the <g> with transform if present
    g_transform = re.search(r'<g transform="([^"]+)"', svg_content)
    transform_attr = f' transform="{g_transform.group(1)}"' if g_transform else ""

    # Build clean SVG using potrace's coordinate system
    pvb = potrace_vb.group(1) if potrace_vb else f"0 0 {w} {h}"

    if args.viewbox:
        final_vb = viewbox
    else:
        final_vb = pvb

    lines = [
        f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="{final_vb}">',
    ]

    if args.bg:
        bw_val = final_vb.split()[2]
        bh_val = final_vb.split()[3]
        lines.append(f'  <rect width="{bw_val}" height="{bh_val}" rx="{args.bg_rx}" fill="{args.bg}"/>')

    # If we need to rescale from potrace coords to our viewBox
    pvb_parts = pvb.split()
    pvb_w = float(pvb_parts[2])
    pvb_h = float(pvb_parts[3])
    fvb_parts = final_vb.split()
    fvb_w = float(fvb_parts[2])
    fvb_h = float(fvb_parts[3])

    needs_scale = abs(pvb_w - fvb_w) > 0.1 or abs(pvb_h - fvb_h) > 0.1
    sx = fvb_w / pvb_w if pvb_w else 1
    sy = fvb_h / pvb_h if pvb_h else 1

    if needs_scale:
        combined_transform = f"scale({sx},{sy})"
        if transform_attr:
            # Combine: first potrace's transform, then our scale
            orig = g_transform.group(1) if g_transform else ""
            combined_transform = f"scale({sx},{sy}) {orig}"
        lines.append(f'  <g transform="{combined_transform}">')
    elif transform_attr:
        lines.append(f'  <g{transform_attr}>')

    for path_d in paths:
        lines.append(f'    <path d="{path_d}" fill="{args.fill}"/>')

    if needs_scale or transform_attr:
        lines.append("  </g>")

    lines.append("</svg>")
    lines.append("")

    Path(args.output).write_text("\n".join(lines))

    # Cleanup
    Path(pbm_path).unlink(missing_ok=True)
    Path(svg_tmp).unlink(missing_ok=True)

    print(f"Traced {args.input} -> {args.output}")


if __name__ == "__main__":
    main()
