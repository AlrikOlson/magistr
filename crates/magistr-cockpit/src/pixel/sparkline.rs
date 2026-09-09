//! A series of values as a line over a filled area.

use tiny_skia::{
    Color, FillRule, LineCap, LineJoin, Paint, PathBuilder, PixmapMut, Stroke, Transform,
};

use super::painter::Painter;

/// The line the fill sits under is this fraction of the pixmap height
/// wide, clamped to whole pixels.
const STROKE_FRACTION: f32 = 1.0 / 12.0;
/// Vertical padding so the stroke's outer edge stays inside the pixmap.
const PADDING_FRACTION: f32 = 1.0 / 10.0;

/// A sparkline: the values scaled to the pixmap, joined by a stroked
/// polyline over a translucent area of the same colour, on an opaque
/// background (paint it into a [`PixelSurface::new`](super::PixelSurface)
/// surface).
///
/// Fewer than two values draw the background and a flat line at the
/// middle (one value) or nothing else (none).
#[derive(Clone, Debug, PartialEq)]
pub struct Sparkline {
    values: Vec<f32>,
    line: Color,
    background: Color,
    area: bool,
    sweep: Option<f32>,
}

impl Sparkline {
    /// A sparkline of `values` in a light line on a dark background with
    /// the area filled.
    #[must_use]
    pub fn new(values: impl Into<Vec<f32>>) -> Self {
        Self {
            values: values.into(),
            line: Color::from_rgba8(122, 162, 247, 255),
            background: Color::from_rgba8(26, 27, 38, 255),
            area: true,
            sweep: None,
        }
    }

    /// One frame of the sweep that draws the line in from the left:
    /// only the first `t` of the width (`0..1`) is revealed, the line's
    /// head marked by a dot. `None` — or `1` and beyond — draws it
    /// whole; `0` draws nothing but the background. The frame is a pure
    /// function of `t`, so the runtime's tick can step it.
    #[must_use]
    pub fn sweep(mut self, t: Option<f32>) -> Self {
        self.sweep = t.map(|t| t.clamp(0.0, 1.0)).filter(|t| *t < 1.0);
        self
    }

    /// `points` cut at the sweep's edge — the last kept point
    /// interpolated onto the edge so the head sits exactly there —
    /// or all of them when there is no sweep.
    fn swept(&self, points: Vec<(f32, f32)>, width: f32) -> Vec<(f32, f32)> {
        let Some(t) = self.sweep else {
            return points;
        };
        if t <= 0.0 {
            return Vec::new();
        }
        let edge = t * width;
        let mut kept: Vec<(f32, f32)> =
            points.iter().copied().take_while(|p| p.0 <= edge).collect();
        if let Some(&(x1, y1)) = points.get(kept.len())
            && let Some(&(x0, y0)) = kept.last()
            && x1 > x0
        {
            let u = (edge - x0) / (x1 - x0);
            kept.push((edge, y0 + (y1 - y0) * u));
        }
        kept
    }

    /// The line colour (the area is the same colour at a quarter alpha).
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

    /// Whether the area under the line is filled.
    #[must_use]
    pub const fn area(mut self, area: bool) -> Self {
        self.area = area;
        self
    }

    /// The values.
    #[must_use]
    pub fn values(&self) -> &[f32] {
        &self.values
    }

    /// Each value's pixel position: x spread over the width, y from the
    /// bottom padding (minimum) to the top padding (maximum).
    fn points(&self, width: f32, height: f32) -> Vec<(f32, f32)> {
        let finite: Vec<f32> = self
            .values
            .iter()
            .copied()
            .filter(|v| v.is_finite())
            .collect();
        let (min, max) = finite
            .iter()
            .fold((f32::INFINITY, f32::NEG_INFINITY), |(lo, hi), &v| {
                (lo.min(v), hi.max(v))
            });
        let padding = (height * PADDING_FRACTION).max(1.0);
        let span = height - 2.0 * padding;
        let range = max - min;
        let step = if finite.len() > 1 {
            width / (finite.len() - 1) as f32
        } else {
            0.0
        };
        finite
            .iter()
            .enumerate()
            .map(|(i, &v)| {
                let unit = if range > 0.0 { (v - min) / range } else { 0.5 };
                let x = if finite.len() > 1 {
                    i as f32 * step
                } else {
                    width / 2.0
                };
                (x, height - padding - unit * span)
            })
            .collect()
    }
}

impl Painter for Sparkline {
    fn paint(&self, pixmap: &mut PixmapMut<'_>) {
        pixmap.fill(self.background);
        let (width, height) = (pixmap.width() as f32, pixmap.height() as f32);
        let mut points = self.points(width, height);
        if points.is_empty() {
            return;
        }
        if points.len() == 1 {
            let y = points[0].1;
            points = vec![(0.0, y), (width, y)];
        }
        let points = self.swept(points, width);
        if points.is_empty() {
            return;
        }
        let mut paint = Paint {
            anti_alias: true,
            ..Paint::default()
        };
        if self.area {
            let mut area = PathBuilder::new();
            area.move_to(points[0].0, height);
            for &(x, y) in &points {
                area.line_to(x, y);
            }
            area.line_to(points[points.len() - 1].0, height);
            area.close();
            if let Some(path) = area.finish() {
                let mut fill = self.line;
                fill.set_alpha(0.25);
                paint.set_color(fill);
                pixmap.fill_path(
                    &path,
                    &paint,
                    FillRule::Winding,
                    Transform::identity(),
                    None,
                );
            }
        }
        let mut line = PathBuilder::new();
        line.move_to(points[0].0, points[0].1);
        for &(x, y) in &points[1..] {
            line.line_to(x, y);
        }
        let stroke_width = (height * STROKE_FRACTION).clamp(1.0, 4.0);
        if let Some(path) = line.finish() {
            paint.set_color(self.line);
            let stroke = Stroke {
                width: stroke_width,
                line_cap: LineCap::Round,
                line_join: LineJoin::Round,
                ..Stroke::default()
            };
            pixmap.stroke_path(&path, &paint, &stroke, Transform::identity(), None);
        }
        if self.sweep.is_some()
            && let Some(&(x, y)) = points.last()
            && let Some(head) = PathBuilder::from_circle(x, y, stroke_width * 1.5)
        {
            paint.set_color(self.line);
            pixmap.fill_path(
                &head,
                &paint,
                FillRule::Winding,
                Transform::identity(),
                None,
            );
        }
    }
}

#[cfg(test)]
#[path = "sparkline.tests.rs"]
mod tests;
