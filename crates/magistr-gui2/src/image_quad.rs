//! A stream image at its own pixels: where its quad goes, and what is
//! admitted to the atlas for it.

use magistr_cockpit::layout::PxRect;
use magistr_cockpit::turn::StreamImage;
use qs_gpu::atlas::RgbaImage;
use tiny_skia::{FilterQuality, IntSize, Pixmap, PixmapPaint, Transform};

/// Texels a picture has to leave free on each side of the colour page:
/// the atlas pads every entry by a gutter, and a picture exactly the
/// page's width has nowhere to put it.
const PAGE_MARGIN: u32 = 4;

/// The quad a stream image draws as: the part of it on screen, in
/// pixels, and the slice of its atlas entry under that part.
///
/// The picture is drawn at **its own pixels** from the top-left of the
/// box the turn's footprint reserved for it, and scaled down — never up,
/// aspect kept — only when the footprint was shrunk to fit the stage's
/// width or the row cap. That is the whole difference from the terminal
/// path, which paints the picture *into* its cells: a cell is the unit
/// of a terminal's screen and a pixel is the unit of a window's, and a
/// picture resampled onto a cell grid would be blurred by up to a
/// cell's rounding for nothing.
///
/// The rows that scrolled off take their share of the entry with them,
/// so a placement half past the top of the stage shows the lower half of
/// the picture at the same pixels it had before — the picture moves
/// under the window rather than squashing into it — and no pixel is
/// re-rasterized or re-admitted for a scroll.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ImageQuad {
    /// Where the visible part of the picture lands.
    pub rect: PxRect,
    /// The atlas entry's `[u0, v0, u1, v1]`, cut to that part.
    pub uv: [f32; 4],
}

impl ImageQuad {
    /// The quad for a picture `image` (its `width × height` in pixels)
    /// admitted under `uv`, placed in `placement` — the footprint's
    /// whole pixel box, every row of it, on screen or not — and cut to
    /// `band`, the part of that box the stage shows this frame.
    ///
    /// `None` when nothing of the picture is inside `band`: the rows on
    /// screen are below a picture shorter than its box, or the box has
    /// no size.
    #[must_use]
    pub fn place(placement: PxRect, band: PxRect, image: (u32, u32), uv: [f32; 4]) -> Option<Self> {
        let (width, height) = (image.0 as f32, image.1 as f32);
        if width <= 0.0 || height <= 0.0 || placement.is_empty() {
            return None;
        }
        // Shrunk by whichever bound bites, never grown: the footprint
        // keeps the aspect, so at most one bound is short and the other
        // leaves a sliver of the box unpainted rather than stretching.
        let scale = (placement.width / width)
            .min(placement.height / height)
            .min(1.0);
        let drawn = PxRect::new(placement.x, placement.y, width * scale, height * scale);
        let visible = drawn.intersection(band);
        if visible.is_empty() {
            return None;
        }
        let [u0, v0, u1, v1] = uv;
        let per_x = (u1 - u0) / drawn.width;
        let per_y = (v1 - v0) / drawn.height;
        Some(Self {
            rect: visible,
            uv: [
                u0 + (visible.x - drawn.x) * per_x,
                v0 + (visible.y - drawn.y) * per_y,
                u0 + (visible.right() - drawn.x) * per_x,
                v0 + (visible.bottom() - drawn.y) * per_y,
            ],
        })
    }

    /// The pixels the atlas is handed for `image`, on a colour page
    /// `page` texels on a side: the picture's own bytes whenever the
    /// page holds them, else the picture scaled down to fit the page
    /// with its aspect kept.
    ///
    /// Fitting rather than refusing, because the atlas refuses an
    /// oversize picture *for ever* (`rejected_oversize` is not a
    /// deferral) and a screenshot larger than the page is exactly the
    /// one a user most wants to see. A picture the page cannot hold at
    /// native size is shown at the largest size it can; the quad then
    /// draws it at its native size from a smaller entry, which is the
    /// same sampling the fit-to-width case already does.
    #[must_use]
    pub fn admission(image: &StreamImage, page: u32) -> RgbaImage {
        let (width, height) = (u32::from(image.width()), u32::from(image.height()));
        let limit = page.saturating_sub(PAGE_MARGIN).max(1);
        let own = || RgbaImage {
            width,
            height,
            rgba: image.rgba().to_vec(),
        };
        if width <= limit && height <= limit {
            return own();
        }
        let scale = (limit as f32 / width as f32).min(limit as f32 / height as f32);
        let fitted = (
            ((width as f32 * scale).floor() as u32).clamp(1, limit),
            ((height as f32 * scale).floor() as u32).clamp(1, limit),
        );
        fit(image.rgba(), (width, height), fitted).unwrap_or_else(own)
    }
}

/// `rgba` (straight alpha, `from.0 × from.1`) resampled to `to` with a
/// bilinear filter, back as straight alpha.
///
/// tiny-skia composites premultiplied pixels, so the bytes are
/// premultiplied on the way in and demultiplied on the way out — the
/// atlas wants them straight (see [`RgbaImage`] for why the multiply
/// belongs downstream of admission).
fn fit(rgba: &[u8], from: (u32, u32), to: (u32, u32)) -> Option<RgbaImage> {
    let source = IntSize::from_wh(from.0, from.1)?;
    if rgba.len() != (from.0 as usize) * (from.1 as usize) * 4 {
        return None;
    }
    let premultiplied: Vec<u8> = rgba
        .chunks_exact(4)
        .flat_map(|px| {
            let alpha = u32::from(px[3]);
            let scale = |c: u8| ((u32::from(c) * alpha) / 255) as u8;
            [scale(px[0]), scale(px[1]), scale(px[2]), px[3]]
        })
        .collect();
    let source = Pixmap::from_vec(premultiplied, source)?;
    let mut target = Pixmap::new(to.0, to.1)?;
    target.draw_pixmap(
        0,
        0,
        source.as_ref(),
        &PixmapPaint {
            quality: FilterQuality::Bilinear,
            ..PixmapPaint::default()
        },
        Transform::from_scale(to.0 as f32 / from.0 as f32, to.1 as f32 / from.1 as f32),
        None,
    );
    let mut out = Vec::with_capacity(target.pixels().len() * 4);
    for px in target.pixels() {
        let straight = px.demultiply();
        out.extend_from_slice(&[
            straight.red(),
            straight.green(),
            straight.blue(),
            straight.alpha(),
        ]);
    }
    Some(RgbaImage {
        width: to.0,
        height: to.1,
        rgba: out,
    })
}

#[cfg(test)]
#[path = "image_quad.tests.rs"]
mod tests;
