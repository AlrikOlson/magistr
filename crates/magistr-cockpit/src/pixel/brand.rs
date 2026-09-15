//! The product mark as a vector.

use tiny_skia::{
    Color, FillRule, LineCap, LineJoin, Paint, PathBuilder, PixmapMut, Stroke, Transform,
};

use super::painter::Painter;

/// The connected route that forms Magistr's `M` mark in unit-square
/// coordinates. It matches the website mark and remains legible in a
/// single terminal cell.
const ROUTE: [(f32, f32); 5] = [
    (-0.42, 0.40),
    (-0.42, -0.40),
    (0.0, 0.16),
    (0.42, -0.40),
    (0.42, 0.40),
];
const NODES: [(f32, f32); 3] = [(-0.42, -0.40), (0.0, 0.16), (0.42, -0.40)];
const STROKE_FRACTION: f32 = 0.14;
const NODE_RADIUS_FRACTION: f32 = 0.105;

/// Magistr's route mark for the header brand slot, drawn as a vector on a
/// transparent background so the header's own colour shows around it.
///
/// The mark scales to the shorter side of its pixmap, so a one-cell slot
/// gets a one-cell mark whatever the terminal's cell size is.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Brand {
    color: Color,
    node_colors: [Color; 3],
}

impl Brand {
    /// The mark in `(r, g, b)` — the pen the header's brand text would
    /// have worn. The route follows that colour while its nodes use the
    /// product palette so the mark stays recognizable in the terminal.
    #[must_use]
    pub fn new(r: u8, g: u8, b: u8) -> Self {
        Self {
            color: Color::from_rgba8(r, g, b, 255),
            node_colors: [
                Color::from_rgba8(103, 232, 249, 255),
                Color::from_rgba8(167, 139, 250, 255),
                Color::from_rgba8(251, 191, 36, 255),
            ],
        }
    }
}

impl Painter for Brand {
    fn paint(&self, pixmap: &mut PixmapMut<'_>) {
        pixmap.fill(Color::TRANSPARENT);
        let (width, height) = (pixmap.width() as f32, pixmap.height() as f32);
        let side = width.min(height) * 0.78;
        let (cx, cy) = (width / 2.0, height / 2.0);
        let at = |(x, y): (f32, f32)| (x.mul_add(side, cx), y.mul_add(side, cy));

        let mut path = PathBuilder::new();
        for (index, point) in ROUTE.into_iter().enumerate() {
            let (x, y) = at(point);
            if index == 0 {
                path.move_to(x, y);
            } else {
                path.line_to(x, y);
            }
        }
        let Some(path) = path.finish() else {
            return;
        };
        let mut paint = Paint {
            anti_alias: true,
            ..Paint::default()
        };
        paint.set_color(self.color);
        let stroke = Stroke {
            width: side * STROKE_FRACTION,
            line_cap: LineCap::Round,
            line_join: LineJoin::Round,
            ..Stroke::default()
        };
        pixmap.stroke_path(&path, &paint, &stroke, Transform::identity(), None);

        let node_radius = side * NODE_RADIUS_FRACTION;
        for (index, (x, y)) in NODES.into_iter().enumerate() {
            let Some(circle) =
                PathBuilder::from_circle(x.mul_add(side, cx), y.mul_add(side, cy), node_radius)
            else {
                continue;
            };
            paint.set_color(self.node_colors[index]);
            pixmap.fill_path(
                &circle,
                &paint,
                FillRule::Winding,
                Transform::identity(),
                None,
            );
        }
    }
}

#[cfg(test)]
#[path = "brand.tests.rs"]
mod tests;
