//! The product mark as a vector.

use tiny_skia::{Color, FillRule, Paint, PathBuilder, PixmapMut, Transform};

use super::painter::Painter;

/// The mark's outline in a unit square centred on the pixmap: an
/// ascending peak, cut by the crossbar slot below.
const PEAK: [(f32, f32); 3] = [(0.0, -0.44), (0.42, 0.40), (-0.42, 0.40)];
/// The slot through the peak — an `A`'s crossbar, drawn as a hole by
/// the even-odd rule.
const SLOT: [(f32, f32); 4] = [(-0.19, 0.09), (0.19, 0.09), (0.19, 0.21), (-0.19, 0.21)];

/// The magistr mark for the header's brand slot: the same ascending peak
/// the `▲` glyph stands for, drawn as a vector on a transparent
/// background — paint it into a
/// [`PixelSurface::with_alpha`](super::PixelSurface::with_alpha) surface
/// so the header's own colour shows around it.
///
/// The mark scales to the shorter side of its pixmap, so a one-cell slot
/// gets a one-cell mark whatever the terminal's cell size is.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Brand {
    color: Color,
}

impl Brand {
    /// The mark in `(r, g, b)` — the pen the header's brand text would
    /// have worn.
    #[must_use]
    pub fn new(r: u8, g: u8, b: u8) -> Self {
        Self {
            color: Color::from_rgba8(r, g, b, 255),
        }
    }
}

impl Painter for Brand {
    fn paint(&self, pixmap: &mut PixmapMut<'_>) {
        pixmap.fill(Color::TRANSPARENT);
        let (width, height) = (pixmap.width() as f32, pixmap.height() as f32);
        let side = width.min(height);
        let (cx, cy) = (width / 2.0, height / 2.0);
        let at = |(x, y): (f32, f32)| (x.mul_add(side, cx), y.mul_add(side, cy));
        let mut path = PathBuilder::new();
        for (i, point) in PEAK.into_iter().enumerate() {
            let (x, y) = at(point);
            if i == 0 {
                path.move_to(x, y);
            } else {
                path.line_to(x, y);
            }
        }
        path.close();
        for (i, point) in SLOT.into_iter().enumerate() {
            let (x, y) = at(point);
            if i == 0 {
                path.move_to(x, y);
            } else {
                path.line_to(x, y);
            }
        }
        path.close();
        let Some(path) = path.finish() else {
            return;
        };
        let mut paint = Paint {
            anti_alias: true,
            ..Paint::default()
        };
        paint.set_color(self.color);
        pixmap.fill_path(
            &path,
            &paint,
            FillRule::EvenOdd,
            Transform::identity(),
            None,
        );
    }
}

#[cfg(test)]
#[path = "brand.tests.rs"]
mod tests;
