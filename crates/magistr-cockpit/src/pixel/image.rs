//! An agent's image, scaled into the cells the stream gave it.

use tiny_skia::{Color, FilterQuality, IntSize, Pixmap, PixmapMut, PixmapPaint, Transform};

use super::painter::Painter;

/// One decoded image drawn to fill its surface: scaled to the surface's
/// pixels with a bilinear filter, on an opaque background so a
/// transparent screenshot reads against the stage rather than the
/// terminal's own colour.
///
/// The aspect is already settled by the placement the stream measured
/// ([`StreamImage::footprint`](crate::turn::StreamImage::footprint)), so
/// this scales to the whole surface: any leftover distortion is a
/// rounding of the image onto the cell grid, at most half a cell.
#[derive(Clone, Debug)]
pub struct Image {
    source: Option<Pixmap>,
    background: Color,
    first: u16,
    count: u16,
    total: u16,
}

impl Image {
    /// An image from straight (non-premultiplied) RGBA pixels of
    /// `width × height`. Pixels that do not describe that rectangle
    /// leave the painter with nothing but its background.
    #[must_use]
    pub fn new(rgba: &[u8], width: u16, height: u16) -> Self {
        Self {
            source: source(rgba, width, height),
            background: Color::from_rgba8(26, 27, 38, 255),
            first: 0,
            count: 1,
            total: 1,
        }
    }

    /// Draw only rows `first..first + count` of a placement `total` rows
    /// tall — the part of the image the stage's window still shows when
    /// the rest has scrolled past. The surface is those rows; the image
    /// is scaled as if the whole placement were on screen and shifted up
    /// by the rows above it.
    #[must_use]
    pub const fn rows(mut self, first: u16, count: u16, total: u16) -> Self {
        if count > 0 && total > 0 && first < total {
            self.first = first;
            self.count = count;
            self.total = total;
        }
        self
    }

    /// The colour under the image — the stage's ground.
    #[must_use]
    pub fn background(mut self, r: u8, g: u8, b: u8) -> Self {
        self.background = Color::from_rgba8(r, g, b, 255);
        self
    }

    /// Whether the pixels made a drawable image.
    #[must_use]
    pub const fn is_drawable(&self) -> bool {
        self.source.is_some()
    }
}

impl Painter for Image {
    fn paint(&self, pixmap: &mut PixmapMut<'_>) {
        pixmap.fill(self.background);
        let Some(source) = &self.source else {
            return;
        };
        let (width, height) = (pixmap.width() as f32, pixmap.height() as f32);
        if width <= 0.0 || height <= 0.0 {
            return;
        }
        // The surface holds `count` of the placement's `total` rows, so
        // the whole image would be this tall; it is drawn at that size
        // and shifted up by the rows above the window.
        let full = height * f32::from(self.total) / f32::from(self.count);
        let scale_x = width / source.width() as f32;
        let scale_y = full / source.height() as f32;
        let shift = -full * f32::from(self.first) / f32::from(self.total);
        let placement = Transform::from_row(scale_x, 0.0, 0.0, scale_y, 0.0, shift);
        pixmap.draw_pixmap(
            0,
            0,
            source.as_ref(),
            &PixmapPaint {
                quality: FilterQuality::Bilinear,
                ..PixmapPaint::default()
            },
            placement,
            None,
        );
    }
}

/// The straight-alpha pixels as the premultiplied pixmap tiny-skia
/// composites from; `None` when they do not fill `width × height`.
fn source(rgba: &[u8], width: u16, height: u16) -> Option<Pixmap> {
    let size = IntSize::from_wh(u32::from(width), u32::from(height))?;
    let expected = usize::from(width) * usize::from(height) * 4;
    if rgba.len() != expected {
        return None;
    }
    // Truncating rather than rounding keeps every channel at or under
    // its alpha, which is what makes a pixel validly premultiplied.
    let premultiplied: Vec<u8> = rgba
        .chunks_exact(4)
        .flat_map(|px| {
            let alpha = u32::from(px[3]);
            let scale = |c: u8| ((u32::from(c) * alpha) / 255) as u8;
            [scale(px[0]), scale(px[1]), scale(px[2]), px[3]]
        })
        .collect();
    Pixmap::from_vec(premultiplied, size)
}

#[cfg(test)]
#[path = "image.tests.rs"]
mod tests;
