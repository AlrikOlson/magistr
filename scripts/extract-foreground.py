#!/usr/bin/env python3
"""Extract foreground from green-screen AI-generated images.

3-tier pipeline for converting mcp-image output (on #00FF00 background)
into transparent PNGs:

  Tier 1: CorridorKey  — neural color unmixing (best quality, needs GPU)
  Tier 2: rembg        — BiRefNet segmentation (good quality, CPU ok)
  Tier 3: Algorithmic  — LAB delta-E chroma key (basic, zero deps beyond PIL/numpy)

Usage:
  python scripts/extract-foreground.py input.jpg output.png
  python scripts/extract-foreground.py input.jpg output.png --size 512x512
  python scripts/extract-foreground.py input.jpg output.png --tier corridorkey
  python scripts/extract-foreground.py input.jpg output.png --sizes 16,32,180,192,512

The script auto-selects the best available tier unless --tier is specified.
"""

from __future__ import annotations

import argparse
import sys
from pathlib import Path

import numpy as np
from PIL import Image


# ---------------------------------------------------------------------------
# Tier detection
# ---------------------------------------------------------------------------

def _corridorkey_available() -> bool:
    """Check if CorridorKey is importable and has a checkpoint."""
    corridorkey_root = Path.home() / "Code" / "CorridorKey"
    if not corridorkey_root.exists():
        return False
    try:
        # Add to path so we can import
        sys.path.insert(0, str(corridorkey_root))
        import CorridorKeyModule.engine_factory  # noqa: F401
        return True
    except ImportError:
        return False
    finally:
        if str(corridorkey_root) in sys.path:
            sys.path.remove(str(corridorkey_root))


def _rembg_available() -> bool:
    try:
        import rembg  # noqa: F401
        return True
    except ImportError:
        return False


def detect_best_tier() -> str:
    if _corridorkey_available():
        return "corridorkey"
    if _rembg_available():
        return "rembg"
    return "chroma"


# ---------------------------------------------------------------------------
# Tier 1: BiRefNet alpha + chroma color unmixing (CorridorKey's BiRefNet module)
# ---------------------------------------------------------------------------

def extract_corridorkey(img: Image.Image) -> Image.Image:
    """BiRefNet neural alpha mask + algorithmic green screen color unmixing.

    Uses CorridorKey's BiRefNet module for a high-quality soft alpha mask,
    then performs linear color unmixing to reconstruct the true foreground
    color without green contamination. Best quality for AI-generated assets
    on green screen backgrounds.
    """
    corridorkey_root = Path.home() / "Code" / "CorridorKey"
    sys.path.insert(0, str(corridorkey_root))

    try:
        from BiRefNetModule.wrapper import BiRefNetHandler
    finally:
        if str(corridorkey_root) in sys.path:
            sys.path.remove(str(corridorkey_root))

    rgb = np.array(img.convert("RGB")).astype(np.float32) / 255.0

    # --- Step 1: Generate soft alpha mask with BiRefNet ---
    print("  [birefnet] generating neural alpha mask...")
    import platform
    if sys.platform == "darwin" and platform.machine() == "arm64":
        device = "mps"
    else:
        device = "cuda" if _cuda_available() else "cpu"

    birefnet = BiRefNetHandler(device=device, usage="General")

    from torchvision import transforms as T
    import torch

    preprocessor_res = birefnet.resolution or (1024, 1024)
    transform = T.Compose([
        T.Resize(preprocessor_res),
        T.ToTensor(),
        T.Normalize([0.485, 0.456, 0.406], [0.229, 0.224, 0.225]),
    ])
    pil_rgb = img.convert("RGB")
    input_tensor = transform(pil_rgb).unsqueeze(0).to(device)
    if hasattr(birefnet, 'birefnet'):
        input_tensor = input_tensor.half()
        with torch.no_grad():
            preds = birefnet.birefnet(input_tensor)[-1].sigmoid().cpu()
        pred = preds[0].squeeze()
        mask_pil = T.ToPILImage()(pred.float())
        mask_pil = mask_pil.resize((img.width, img.height), Image.LANCZOS)
        alpha = np.array(mask_pil).astype(np.float32) / 255.0
    else:
        alpha = _green_distance_mask(rgb)

    birefnet.cleanup()
    del birefnet

    # --- Step 2: Refine alpha with chroma distance ---
    # BiRefNet gives shape, but chroma distance catches green-tinted edge pixels
    # that BiRefNet might keep. Combine: min(birefnet_alpha, chroma_alpha) at edges.
    print("  [refine] combining neural alpha with chroma distance...")
    chroma_alpha = _green_distance_mask(rgb)

    # Detect edge band: pixels where BiRefNet alpha is between 0.05 and 0.95
    edge_band = (alpha > 0.05) & (alpha < 0.95)
    # In edge band, use the more conservative (lower) of the two alphas
    refined_alpha = alpha.copy()
    refined_alpha[edge_band] = np.minimum(alpha[edge_band], chroma_alpha[edge_band])

    # Slight erosion to pull boundary inward (kills outermost green fringe)
    try:
        import cv2
        kernel = cv2.getStructuringElement(cv2.MORPH_ELLIPSE, (3, 3))
        alpha_u8_tmp = (refined_alpha * 255).astype(np.uint8)
        alpha_u8_tmp = cv2.erode(alpha_u8_tmp, kernel, iterations=1)
        # Gentle blur on the edge to smooth the erosion
        alpha_u8_tmp = cv2.GaussianBlur(alpha_u8_tmp, (3, 3), 0.5)
        refined_alpha = alpha_u8_tmp.astype(np.float32) / 255.0
    except ImportError:
        pass

    alpha = refined_alpha

    # --- Step 3: Linear color unmixing ---
    # Compositing eq: pixel = fg * a + green * (1 - a)
    # Solve: fg = (pixel - green * (1 - a)) / a
    print("  [unmix] performing linear color unmixing...")
    green_ref = np.array([0.0, 1.0, 0.0], dtype=np.float32)

    safe_alpha = np.maximum(alpha[:, :, np.newaxis], 1e-6)
    bg_contrib = green_ref[np.newaxis, np.newaxis, :] * (1.0 - safe_alpha)
    fg = (rgb - bg_contrib) / safe_alpha
    fg = np.clip(fg, 0.0, 1.0)

    # --- Step 4: Despill (from CorridorKey's color_utils) ---
    # Clamp green channel to average of R and B, redistribute excess
    # This kills any remaining green tint in semi-transparent edge pixels
    print("  [despill] removing green spill from edge pixels...")
    r, g, b = fg[..., 0], fg[..., 1], fg[..., 2]
    limit = (r + b) / 2.0
    spill = np.maximum(g - limit, 0.0)
    fg[..., 0] = r + spill * 0.5  # redistribute to red
    fg[..., 1] = g - spill         # remove from green
    fg[..., 2] = b + spill * 0.5  # redistribute to blue
    fg = np.clip(fg, 0.0, 1.0)

    # Zero out fully transparent pixels
    fg[alpha < 0.01] = 0.0

    # Compose RGBA
    fg_u8 = (fg * 255).astype(np.uint8)
    alpha_u8 = (alpha * 255).astype(np.uint8)
    rgba = np.dstack([fg_u8, alpha_u8])
    return Image.fromarray(rgba, "RGBA")


def _cuda_available() -> bool:
    try:
        import torch
        return torch.cuda.is_available()
    except ImportError:
        return False


# ---------------------------------------------------------------------------
# Tier 2: rembg — BiRefNet segmentation
# ---------------------------------------------------------------------------

def extract_rembg(img: Image.Image) -> Image.Image:
    """Use rembg with BiRefNet model for background removal."""
    print("  [rembg] running BiRefNet segmentation...")
    from rembg import remove, new_session

    session = new_session("birefnet-general")
    result = remove(img.convert("RGB"), session=session)
    return result.convert("RGBA")


# ---------------------------------------------------------------------------
# Tier 3: Algorithmic chroma key — LAB delta-E + linear color unmixing
# ---------------------------------------------------------------------------

def extract_chroma(img: Image.Image) -> Image.Image:
    """Pure algorithmic green screen removal with color unmixing.

    1. Convert to CIELAB, compute delta-E from pure green
    2. Build soft alpha from distance (smooth transition)
    3. Unmix foreground color: fg = (pixel - (1-a)*green) / a
    4. Edge refinement via morphological ops
    """
    print("  [chroma] running algorithmic green screen removal...")
    rgb = np.array(img.convert("RGB")).astype(np.float32) / 255.0
    h, w = rgb.shape[:2]

    # Green screen reference in sRGB [0,1]
    green_ref = np.array([0.0, 1.0, 0.0], dtype=np.float32)

    # --- Convert to CIELAB for perceptual distance ---
    lab = _srgb_to_lab(rgb)
    green_lab = _srgb_to_lab(green_ref.reshape(1, 1, 3))[0, 0]

    # Delta-E (CIE76) — Euclidean distance in LAB space
    delta_e = np.sqrt(np.sum((lab - green_lab) ** 2, axis=-1))

    # --- Build soft alpha from distance ---
    # Pixels with delta_e < inner_threshold are fully transparent (green bg)
    # Pixels with delta_e > outer_threshold are fully opaque (foreground)
    # In between: smooth gradient
    inner_threshold = 30.0   # pure green and close shades
    outer_threshold = 60.0   # clearly not green

    alpha = np.clip((delta_e - inner_threshold) / (outer_threshold - inner_threshold), 0.0, 1.0)

    # --- Morphological edge refinement ---
    # Erode alpha slightly to remove green fringe at boundary
    alpha_u8 = (alpha * 255).astype(np.uint8)
    try:
        import cv2
        kernel = cv2.getStructuringElement(cv2.MORPH_ELLIPSE, (3, 3))
        alpha_u8 = cv2.erode(alpha_u8, kernel, iterations=1)
        alpha_u8 = cv2.GaussianBlur(alpha_u8, (3, 3), 0)
    except ImportError:
        # No cv2 — skip morphological refinement
        pass

    alpha = alpha_u8.astype(np.float32) / 255.0

    # --- Linear color unmixing ---
    # Compositing equation: pixel = fg * a + bg * (1 - a)
    # Solve for fg: fg = (pixel - bg * (1 - a)) / a
    # Where bg = green_ref
    safe_alpha = np.maximum(alpha[:, :, np.newaxis], 1e-6)
    bg_contrib = green_ref[np.newaxis, np.newaxis, :] * (1.0 - safe_alpha)
    fg = (rgb - bg_contrib) / safe_alpha

    # Clamp — unmixing can produce out-of-range values near edges
    fg = np.clip(fg, 0.0, 1.0)

    # --- Despill: clamp green to avg(R,B), redistribute excess ---
    r, g, b = fg[..., 0], fg[..., 1], fg[..., 2]
    limit = (r + b) / 2.0
    spill = np.maximum(g - limit, 0.0)
    fg[..., 0] = r + spill * 0.5
    fg[..., 1] = g - spill
    fg[..., 2] = b + spill * 0.5
    fg = np.clip(fg, 0.0, 1.0)

    # For fully transparent pixels, zero out fg to avoid noise
    mask_zero = alpha < 0.01
    fg[mask_zero] = 0.0

    # --- Compose RGBA ---
    fg_u8 = (fg * 255).astype(np.uint8)
    alpha_u8_final = (alpha * 255).astype(np.uint8)
    rgba = np.dstack([fg_u8, alpha_u8_final])
    return Image.fromarray(rgba, "RGBA")


def _srgb_to_lab(srgb: np.ndarray) -> np.ndarray:
    """Convert sRGB [0,1] float to CIELAB. Pure numpy, no cv2 needed."""
    # sRGB → linear
    linear = np.where(srgb <= 0.04045, srgb / 12.92, ((srgb + 0.055) / 1.055) ** 2.4)

    # Linear RGB → XYZ (D65)
    r, g, b = linear[..., 0], linear[..., 1], linear[..., 2]
    x = r * 0.4124564 + g * 0.3575761 + b * 0.1804375
    y = r * 0.2126729 + g * 0.7151522 + b * 0.0721750
    z = r * 0.0193339 + g * 0.1191920 + b * 0.9503041

    # XYZ → LAB (D65 reference white)
    xn, yn, zn = 0.95047, 1.0, 1.08883
    fx = _lab_f(x / xn)
    fy = _lab_f(y / yn)
    fz = _lab_f(z / zn)

    L = 116.0 * fy - 16.0
    a = 500.0 * (fx - fy)
    b_ch = 200.0 * (fy - fz)

    return np.stack([L, a, b_ch], axis=-1)


def _lab_f(t: np.ndarray) -> np.ndarray:
    """CIE LAB nonlinear transform."""
    delta = 6.0 / 29.0
    return np.where(t > delta ** 3, np.cbrt(t), t / (3 * delta ** 2) + 4.0 / 29.0)


def _green_distance_mask(rgb: np.ndarray) -> np.ndarray:
    """Simple green-channel distance mask as a fallback alpha hint."""
    green_ref = np.array([0.0, 1.0, 0.0], dtype=np.float32)
    dist = np.sqrt(np.sum((rgb - green_ref) ** 2, axis=-1))
    mask = np.clip(dist / 1.0, 0.0, 1.0)
    return mask


# ---------------------------------------------------------------------------
# Multi-size export
# ---------------------------------------------------------------------------

def export_sizes(img: Image.Image, output_path: Path, sizes: list[int]) -> list[Path]:
    """Export image at multiple square sizes. Returns list of output paths."""
    paths = []
    stem = output_path.stem
    parent = output_path.parent
    suffix = output_path.suffix or ".png"

    for size in sizes:
        resized = img.copy()
        resized.thumbnail((size, size), Image.LANCZOS)
        out = parent / f"{stem}-{size}x{size}{suffix}"
        resized.save(out, "PNG")
        paths.append(out)
        print(f"  exported: {out} ({size}x{size})")

    return paths


# ---------------------------------------------------------------------------
# Main
# ---------------------------------------------------------------------------

TIER_FNS = {
    "corridorkey": extract_corridorkey,
    "rembg": extract_rembg,
    "chroma": extract_chroma,
}


def main():
    parser = argparse.ArgumentParser(
        description="Extract transparent foreground from green-screen AI-generated images.",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog=__doc__,
    )
    parser.add_argument("input", type=Path, help="Input image (JPEG/PNG from mcp-image)")
    parser.add_argument("output", type=Path, help="Output transparent PNG")
    parser.add_argument(
        "--tier",
        choices=["corridorkey", "rembg", "chroma", "auto"],
        default="auto",
        help="Extraction engine (default: auto-detect best available)",
    )
    parser.add_argument(
        "--size",
        type=str,
        default=None,
        help="Resize output to WxH (e.g. 512x512)",
    )
    parser.add_argument(
        "--sizes",
        type=str,
        default=None,
        help="Export at multiple square sizes (e.g. 16,32,180,192,512)",
    )

    args = parser.parse_args()

    if not args.input.exists():
        print(f"Error: input file not found: {args.input}", file=sys.stderr)
        sys.exit(1)

    # Load input
    img = Image.open(args.input)
    print(f"Input: {args.input} ({img.width}x{img.height}, {img.mode})")

    # Select tier
    if args.tier == "auto":
        tier = detect_best_tier()
    else:
        tier = args.tier

    print(f"Engine: {tier}")

    # Extract
    extract_fn = TIER_FNS[tier]
    result = extract_fn(img)

    # Resize if requested
    if args.size:
        w, h = (int(x) for x in args.size.lower().split("x"))
        result = result.resize((w, h), Image.LANCZOS)
        print(f"Resized to {w}x{h}")

    # Save main output
    args.output.parent.mkdir(parents=True, exist_ok=True)
    result.save(args.output, "PNG")
    print(f"Output: {args.output} ({result.width}x{result.height}, RGBA)")

    # Multi-size export
    if args.sizes:
        sizes = [int(s.strip()) for s in args.sizes.split(",")]
        export_sizes(result, args.output, sizes)

    print("Done.")


if __name__ == "__main__":
    main()
