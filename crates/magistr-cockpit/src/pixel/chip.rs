//! A key cap as a drawn chip.

use tiny_skia::{Color, FillRule, Paint, PixmapMut, Transform};

use super::painter::Painter;
use super::rounded::rounded;

/// The cap's inset from the cell's top and bottom, as a fraction of the
/// pixmap height: what makes it read as raised off the bar.
const INSET: f32 = 0.12;
/// The cap's corner radius in pixels.
const RADIUS: f32 = 3.5;

/// A raised key cap under a key's cells: a rounded `fill` over the
/// bar's `ground`, opaque so the rounded-off corners show the bar and
/// never the cell behind — paint it into a
/// [`PixelSurface::new`](super::PixelSurface::new) surface covering the
/// key's text; the text draws over it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Chip {
    fill: Color,
    ground: Color,
}

impl Chip {
    /// A cap in `fill` on `ground`, each `(r, g, b)`.
    #[must_use]
    pub fn new(fill: (u8, u8, u8), ground: (u8, u8, u8)) -> Self {
        Self {
            fill: Color::from_rgba8(fill.0, fill.1, fill.2, 255),
            ground: Color::from_rgba8(ground.0, ground.1, ground.2, 255),
        }
    }
}

impl Painter for Chip {
    fn paint(&self, pixmap: &mut PixmapMut<'_>) {
        pixmap.fill(self.ground);
        let (width, height) = (pixmap.width() as f32, pixmap.height() as f32);
        let inset = height * INSET;
        let Some(cap) = rounded(0.5, inset, width - 0.5, height - inset, RADIUS) else {
            return;
        };
        let mut paint = Paint {
            anti_alias: true,
            ..Paint::default()
        };
        paint.set_color(self.fill);
        pixmap.fill_path(&cap, &paint, FillRule::Winding, Transform::identity(), None);
    }
}

#[cfg(test)]
#[path = "chip.tests.rs"]
mod tests;
