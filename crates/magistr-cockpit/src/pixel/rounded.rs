//! The rounded rectangle every drawn chip, segment and label is made of.

use tiny_skia::{Path, PathBuilder};

/// A rounded rectangle from `(x0, y0)` to `(x1, y1)` with corners of
/// `radius`, the radius capped so the corners never cross; `None` when
/// the box has no area.
#[must_use]
pub fn rounded(x0: f32, y0: f32, x1: f32, y1: f32, radius: f32) -> Option<Path> {
    if x1 <= x0 || y1 <= y0 {
        return None;
    }
    let r = radius.min((x1 - x0) / 2.0).min((y1 - y0) / 2.0).max(0.0);
    let mut path = PathBuilder::new();
    path.move_to(x0 + r, y0);
    path.line_to(x1 - r, y0);
    path.quad_to(x1, y0, x1, y0 + r);
    path.line_to(x1, y1 - r);
    path.quad_to(x1, y1, x1 - r, y1);
    path.line_to(x0 + r, y1);
    path.quad_to(x0, y1, x0, y1 - r);
    path.line_to(x0, y0 + r);
    path.quad_to(x0, y0, x0 + r, y0);
    path.close();
    path.finish()
}

#[cfg(test)]
#[path = "rounded.tests.rs"]
mod tests;
