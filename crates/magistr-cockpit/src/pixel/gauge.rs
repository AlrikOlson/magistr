//! A fraction as a smooth bar.

use tiny_skia::{Color, LineCap, Paint, PathBuilder, PixmapMut, Stroke, Transform};

use super::painter::Painter;

/// The bar's thickness as a fraction of the pixmap height.
const BAR_FRACTION: f32 = 0.42;
/// The track's alpha under the fill's colour.
const TRACK_ALPHA: f32 = 0.22;

/// A gauge: one fraction drawn as a rounded bar over a rounded track,
/// on an opaque background (paint it into a
/// [`PixelSurface::new`](super::PixelSurface) surface).
///
/// This is the cell progress bar's sub-cell twin — the same eased
/// fraction the rail already draws in `▰▱`, resolved to the pixel
/// instead of the cell, so a bar 20 cells wide gains 200 steps.
#[derive(Clone, Debug, PartialEq)]
pub struct Gauge {
    fill: f32,
    line: Color,
    background: Color,
}

impl Gauge {
    /// A gauge showing `fill` (`0..=1`, clamped) in a light bar on a
    /// dark background.
    #[must_use]
    pub fn new(fill: f32) -> Self {
        Self {
            fill: if fill.is_finite() {
                fill.clamp(0.0, 1.0)
            } else {
                0.0
            },
            line: Color::from_rgba8(122, 162, 247, 255),
            background: Color::from_rgba8(26, 27, 38, 255),
        }
    }

    /// The bar's colour (the track is the same colour, faint).
    #[must_use]
    pub fn line(mut self, r: u8, g: u8, b: u8) -> Self {
        self.line = Color::from_rgba8(r, g, b, 255);
        self
    }

    /// The background colour.
    #[must_use]
    pub fn background(mut self, r: u8, g: u8, b: u8) -> Self {
        self.background = Color::from_rgba8(r, g, b, 255);
        self
    }

    /// The fraction it shows.
    #[must_use]
    pub const fn fill(&self) -> f32 {
        self.fill
    }
}

impl Painter for Gauge {
    fn paint(&self, pixmap: &mut PixmapMut<'_>) {
        pixmap.fill(self.background);
        let (width, height) = (pixmap.width() as f32, pixmap.height() as f32);
        let thickness = (height * BAR_FRACTION).clamp(1.0, height);
        // The round caps reach half the thickness past each end, so the
        // bar's line runs inset by that much and the ends stay inside.
        let inset = thickness / 2.0;
        let span = width - 2.0 * inset;
        if span <= 0.0 {
            return;
        }
        let y = height / 2.0;
        let mut paint = Paint {
            anti_alias: true,
            ..Paint::default()
        };
        let stroke = Stroke {
            width: thickness,
            line_cap: LineCap::Round,
            ..Stroke::default()
        };
        let bar = |to: f32| {
            let mut path = PathBuilder::new();
            path.move_to(inset, y);
            path.line_to(to, y);
            path.finish()
        };
        let mut track = self.line;
        track.set_alpha(TRACK_ALPHA);
        paint.set_color(track);
        if let Some(path) = bar(inset + span) {
            pixmap.stroke_path(&path, &paint, &stroke, Transform::identity(), None);
        }
        if self.fill <= 0.0 {
            return;
        }
        paint.set_color(self.line);
        if let Some(path) = bar(inset + span * self.fill) {
            pixmap.stroke_path(&path, &paint, &stroke, Transform::identity(), None);
        }
    }
}

#[cfg(test)]
#[path = "gauge.tests.rs"]
mod tests;
