//! A soft horizontal rule.

use tiny_skia::{Color, LineCap, Paint, PathBuilder, PixmapMut, Stroke, Transform};

use super::painter::Painter;

/// The rule's width in pixels.
const LINE: f32 = 1.5;

/// A horizontal rule through the middle of its cells on a transparent
/// ground — what a labelled divider draws where the cells spelled
/// `──`; paint it into a
/// [`PixelSurface::with_alpha`](super::PixelSurface::with_alpha)
/// surface.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rule {
    ink: Color,
}

impl Rule {
    /// A rule in `(r, g, b)`.
    #[must_use]
    pub fn new(r: u8, g: u8, b: u8) -> Self {
        Self {
            ink: Color::from_rgba8(r, g, b, 255),
        }
    }
}

impl Painter for Rule {
    fn paint(&self, pixmap: &mut PixmapMut<'_>) {
        pixmap.fill(Color::TRANSPARENT);
        let (width, height) = (pixmap.width() as f32, pixmap.height() as f32);
        let y = height / 2.0;
        let mut path = PathBuilder::new();
        path.move_to(1.0, y);
        path.line_to(width - 1.0, y);
        let Some(path) = path.finish() else {
            return;
        };
        let mut paint = Paint {
            anti_alias: true,
            ..Paint::default()
        };
        paint.set_color(self.ink);
        let stroke = Stroke {
            width: LINE,
            line_cap: LineCap::Round,
            ..Stroke::default()
        };
        pixmap.stroke_path(&path, &paint, &stroke, Transform::identity(), None);
    }
}

#[cfg(test)]
#[path = "rule.tests.rs"]
mod tests;
