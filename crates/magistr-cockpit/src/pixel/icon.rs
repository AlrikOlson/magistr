//! Small vector glyphs painted with transparency.

use tiny_skia::{
    Color, FillRule, LineCap, LineJoin, Paint, PathBuilder, PixmapMut, Stroke, Transform,
};

use super::painter::Painter;

/// A status glyph, drawn centred and scaled to the shorter side of its
/// pixmap on a transparent background — paint it into a
/// [`PixelSurface::with_alpha`](super::PixelSurface::with_alpha) surface
/// so the cell background shows through around it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Icon {
    /// A green tick.
    Check,
    /// A red cross.
    Cross,
    /// A blue disc.
    Dot,
    /// A white right-pointing triangle.
    Play,
}

impl Icon {
    /// The glyph's colour.
    #[must_use]
    pub fn color(self) -> Color {
        let (r, g, b) = match self {
            Self::Check => (158, 206, 106),
            Self::Cross => (247, 118, 142),
            Self::Dot => (122, 162, 247),
            Self::Play => (242, 242, 242),
        };
        Color::from_rgba8(r, g, b, 255)
    }
}

impl Painter for Icon {
    fn paint(&self, pixmap: &mut PixmapMut<'_>) {
        pixmap.fill(Color::TRANSPARENT);
        let (width, height) = (pixmap.width() as f32, pixmap.height() as f32);
        let side = width.min(height);
        let (cx, cy) = (width / 2.0, height / 2.0);
        // Coordinates below are in a unit square centred on the pixmap.
        let at = |x: f32, y: f32| (cx + x * side, cy + y * side);
        let mut paint = Paint {
            anti_alias: true,
            ..Paint::default()
        };
        paint.set_color(self.color());
        let stroke = Stroke {
            width: (side / 8.0).max(1.0),
            line_cap: LineCap::Round,
            line_join: LineJoin::Round,
            ..Stroke::default()
        };
        let transform = Transform::identity();
        let mut path = PathBuilder::new();
        match self {
            Self::Check => {
                let (x, y) = at(-0.3, 0.0);
                path.move_to(x, y);
                let (x, y) = at(-0.08, 0.25);
                path.line_to(x, y);
                let (x, y) = at(0.32, -0.25);
                path.line_to(x, y);
            }
            Self::Cross => {
                let (x, y) = at(-0.25, -0.25);
                path.move_to(x, y);
                let (x, y) = at(0.25, 0.25);
                path.line_to(x, y);
                let (x, y) = at(0.25, -0.25);
                path.move_to(x, y);
                let (x, y) = at(-0.25, 0.25);
                path.line_to(x, y);
            }
            Self::Dot => {
                if let Some(circle) = PathBuilder::from_circle(cx, cy, side * 0.3) {
                    pixmap.fill_path(&circle, &paint, FillRule::Winding, transform, None);
                }
                return;
            }
            Self::Play => {
                let (x, y) = at(-0.25, -0.32);
                path.move_to(x, y);
                let (x, y) = at(0.32, 0.0);
                path.line_to(x, y);
                let (x, y) = at(-0.25, 0.32);
                path.line_to(x, y);
                path.close();
                if let Some(path) = path.finish() {
                    pixmap.fill_path(&path, &paint, FillRule::Winding, transform, None);
                }
                return;
            }
        }
        if let Some(path) = path.finish() {
            pixmap.stroke_path(&path, &paint, &stroke, transform, None);
        }
    }
}

#[cfg(test)]
#[path = "icon.tests.rs"]
mod tests;
