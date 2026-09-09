//! Where the tech-tree is looked at from, and how it gets there.

use super::eased::Eased;

/// The Roadmap tab's pan, zoom, and animation offsets.
///
/// [`Camera::drag`] and [`Camera::wheel`] update the settled position
/// immediately. Programmatic movement through [`Camera::glide_to`] stores
/// three residuals in `lag`, one per pan axis and one for zoom. Rendering
/// adds these residuals to the settled values while they ease to zero.
///
/// A drag during a glide moves the settled pan without cancelling the
/// animation. Once the residuals reach zero, rendering matches the settled
/// position exactly. [`Viewport`](crate::turn::Viewport) uses the same
/// approach for scrolling.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Camera {
    pan: (f32, f32),
    zoom: f32,
    /// How far behind the settled camera the paint still is: `(x, y,
    /// zoom)`, each decaying to zero.
    lag: (Eased, Eased, Eased),
}

impl Camera {
    /// Furthest the wheel may zoom in: past this the dots are further
    /// apart than the block is wide and the pan is all there is.
    pub const MAX_ZOOM: f32 = 8.0;

    /// Furthest out: a quarter of the fit still keeps the shape
    /// readable, and below it a hundred dots collapse onto each other.
    pub const MIN_ZOOM: f32 = 0.25;

    /// The camera at the origin, at the fit the viewport picked.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            pan: (0.0, 0.0),
            zoom: 1.0,
            lag: (Eased::new(), Eased::new(), Eased::new()),
        }
    }

    /// The settled pan, in cells.
    #[must_use]
    pub const fn pan(&self) -> (f32, f32) {
        self.pan
    }

    /// The settled zoom: 1.0 is the fit the viewport picked.
    #[must_use]
    pub const fn zoom(&self) -> f32 {
        self.zoom
    }

    /// The pan the paint uses at step `now`: the settled pan plus
    /// whatever of the last focus's move has not caught up yet.
    #[must_use]
    pub fn pan_at(&self, now: f32) -> (f32, f32) {
        (
            self.pan.0 + self.lag.0.at(now),
            self.pan.1 + self.lag.1.at(now),
        )
    }

    /// The zoom the paint uses at step `now`.
    #[must_use]
    pub fn zoom_at(&self, now: f32) -> f32 {
        (self.zoom + self.lag.2.at(now)).max(Self::MIN_ZOOM)
    }

    /// Whether the camera is still catching up with a focus at step
    /// `now` — what keeps the runtime's tick alive for exactly as long
    /// as a glide runs, and no longer.
    #[must_use]
    pub fn is_gliding(&self, now: f32) -> bool {
        self.lag.0.is_settling(now) || self.lag.1.is_settling(now) || self.lag.2.is_settling(now)
    }

    /// A drag on the tech-tree: the view under the pan moves against
    /// the pointer, clamped to `limits` — the overflow past the block,
    /// zero when the whole tree fits. Instant, and the lag is left
    /// alone: whatever a focus still owes keeps decaying underneath, so
    /// the drag itself lands one for one.
    pub fn drag(&mut self, dx: f32, dy: f32, limits: (f32, f32)) {
        self.pan.0 = (self.pan.0 - dx).clamp(0.0, limits.0);
        self.pan.1 = (self.pan.1 - dy).clamp(0.0, limits.1);
    }

    /// A wheel notch at block-relative cell `(x, y)`: the scale
    /// multiplies by `factor` (clamped to [`Camera::MIN_ZOOM`] ..
    /// [`Camera::MAX_ZOOM`]) about that point, so whatever sits under
    /// the pointer stays under it. `margin` is the grid's padding from
    /// the block's edge and `limits` the pan's room after the notch.
    /// Instant, like the drag.
    pub fn wheel(&mut self, factor: f32, x: f32, y: f32, margin: f32, limits: (f32, f32)) {
        let (pan, zoom) = self.zoomed(factor, x, y, margin);
        self.pan = pan;
        self.zoom = zoom;
        self.drag(0.0, 0.0, limits);
    }

    /// Zoom about block-relative cell `(x, y)`, normally the block's center.
    ///
    /// The final position matches [`Camera::wheel`], with rendering eased by
    /// [`Camera::glide_to`]. When `animate` is false, the position changes in one
    /// frame. The caller must clamp the pan against the new scale's limits.
    pub fn zoom_step(&mut self, factor: f32, x: f32, y: f32, margin: f32, now: f32, animate: bool) {
        let (pan, zoom) = self.zoomed(factor, x, y, margin);
        self.glide_to(pan, zoom, now, animate);
    }

    /// Where the camera lands when the scale multiplies by `factor`
    /// about block-relative cell `(x, y)`: the clamped scale, and the
    /// pan that keeps whatever sits at that point sitting there.
    ///
    /// The one copy of the arithmetic, shared by the wheel and the
    /// keyboard step — they differ only in whether the paint follows at
    /// once or glides.
    fn zoomed(&self, factor: f32, x: f32, y: f32, margin: f32) -> ((f32, f32), f32) {
        let zoom = (self.zoom * factor).clamp(Self::MIN_ZOOM, Self::MAX_ZOOM);
        let ratio = zoom / self.zoom;
        // A point at screen `x` sits `x - margin + pan` into the scaled
        // grid; scaling that offset by the same ratio and putting the
        // point back where it was is the whole of zoom-about-a-point.
        (
            (
                (x - margin + self.pan.0).mul_add(ratio, margin - x),
                (y - margin + self.pan.1).mul_add(ratio, margin - y),
            ),
            zoom,
        )
    }

    /// Move the camera to `pan` at `zoom` because something other than
    /// the pointer asked — a chunk selected off the list, a jump from
    /// the trace, a frame putting a chunk in progress.
    ///
    /// The camera lands at once; what glides is the paint, from where
    /// it was at `now` back to nothing over [`Eased::STEPS`]. With
    /// `animate` off — reduced motion — the residual is never filled,
    /// so the move is one frame and no tick is asked for.
    pub fn glide_to(&mut self, pan: (f32, f32), zoom: f32, now: f32, animate: bool) {
        let (was_x, was_y) = self.pan_at(now);
        let was_zoom = self.zoom_at(now);
        self.pan = pan;
        self.zoom = zoom;
        self.lag.0.glide_from(was_x - pan.0, 0.0, now, animate);
        self.lag.1.glide_from(was_y - pan.1, 0.0, now, animate);
        self.lag.2.glide_from(was_zoom - zoom, 0.0, now, animate);
    }
}

impl Default for Camera {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
#[path = "camera.tests.rs"]
mod tests;
