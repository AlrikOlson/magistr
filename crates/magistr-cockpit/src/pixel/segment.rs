//! One segment of a drawn segmented control.

use tiny_skia::{Color, FillRule, Paint, PixmapMut, Transform};

use super::painter::Painter;
use super::rounded::rounded;

/// The track's inset from the cell's top and bottom, as a fraction of
/// the pixmap height.
const TRACK_INSET: f32 = 0.12;
/// The current segment's pill sits this many pixels inside the track.
const PILL_INSET: f32 = 1.5;
/// The track's corner radius at the control's ends, in pixels.
const RADIUS: f32 = 4.0;

/// One segment of a segmented control under its label's cells: the
/// shared track, rounded at the control's outer ends, and — on the
/// current segment — a pill inside it. Opaque on the strip's `ground`
/// so the rounded ends show the strip; paint it into a
/// [`PixelSurface::new`](super::PixelSurface::new) surface covering the
/// label, whose text draws over it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Segment {
    fill: Option<Color>,
    track: Color,
    ground: Color,
    first: bool,
    last: bool,
}

impl Segment {
    /// A segment of a `track` on `ground`, each `(r, g, b)`.
    #[must_use]
    pub fn new(track: (u8, u8, u8), ground: (u8, u8, u8)) -> Self {
        Self {
            fill: None,
            track: Color::from_rgba8(track.0, track.1, track.2, 255),
            ground: Color::from_rgba8(ground.0, ground.1, ground.2, 255),
            first: false,
            last: false,
        }
    }

    /// The current segment: a pill in `(r, g, b)` inside the track.
    #[must_use]
    pub fn fill(mut self, fill: Option<(u8, u8, u8)>) -> Self {
        self.fill = fill.map(|(r, g, b)| Color::from_rgba8(r, g, b, 255));
        self
    }

    /// Which ends of the control this segment closes.
    #[must_use]
    pub const fn ends(mut self, first: bool, last: bool) -> Self {
        self.first = first;
        self.last = last;
        self
    }
}

impl Painter for Segment {
    fn paint(&self, pixmap: &mut PixmapMut<'_>) {
        pixmap.fill(self.ground);
        let (width, height) = (pixmap.width() as f32, pixmap.height() as f32);
        let inset = height * TRACK_INSET;
        let mut paint = Paint {
            anti_alias: true,
            ..Paint::default()
        };
        // The track: rounded where the control ends, and drawn past the
        // pixmap's edge where it continues into the neighbour, so the
        // corners there are clipped square.
        let x0 = if self.first { 0.5 } else { -RADIUS };
        let x1 = if self.last {
            width - 0.5
        } else {
            width + RADIUS
        };
        paint.set_color(self.track);
        if let Some(track) = rounded(x0, inset, x1, height - inset, RADIUS) {
            pixmap.fill_path(
                &track,
                &paint,
                FillRule::Winding,
                Transform::identity(),
                None,
            );
        }
        let Some(fill) = self.fill else {
            return;
        };
        paint.set_color(fill);
        let pill = rounded(
            PILL_INSET,
            inset + PILL_INSET,
            width - PILL_INSET,
            height - inset - PILL_INSET,
            RADIUS - PILL_INSET,
        );
        if let Some(pill) = pill {
            pixmap.fill_path(
                &pill,
                &paint,
                FillRule::Winding,
                Transform::identity(),
                None,
            );
        }
    }
}

#[cfg(test)]
#[path = "segment.tests.rs"]
mod tests;
