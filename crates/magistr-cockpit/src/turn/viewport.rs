//! Which rows of the turn the stage shows.

use crate::cockpit::Eased;

/// A window of `height` rows over `total`: following the tail (the
/// default, and again whenever a scroll reaches the bottom) or pinned to
/// a top row the user scrolled to.
///
/// The window itself moves by whole rows the instant a scroll lands —
/// [`Viewport::top`] is that settled answer, and every clamp, pin and
/// tail-follow rule is decided there. What glides is the *paint*:
/// a scroll leaves the window lagging behind by the rows it
/// just travelled, and that lag eases back to zero over
/// [`Viewport::GLIDE_STEPS`]. Holding the lag rather than an absolute
/// target is what lets a glide survive rows arriving underneath a
/// window that follows the tail — and what makes a settled frame
/// exactly the frame the jump used to paint.
///
/// Two surfaces read the same glide two ways. A window that can paint
/// between rows reads [`Viewport::top_between`], the curve's own
/// fractional row at the frame it is drawing. A cell surface reads
/// [`Viewport::top_at`], a whole row *held* for the tick it shows: it
/// samples the curve a step ahead, because the frame it paints now is
/// what the reader sees until the next tick replaces it — so a notch is
/// on screen the frame it was turned, and a page shows the one
/// intermediate frame a two-step run has, never a strobe of whole rows.
///
/// The pinned top is a row index, and rows move when the turn re-wraps;
/// [`Viewport::hold`] is how the cockpit re-pins the window on the
/// content it was showing (`turn::Anchor`) after every such change.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Viewport {
    top: Option<usize>,
    /// Rows the paint still lags behind the settled top, easing to zero.
    lag: Eased,
}

impl Viewport {
    /// How many rows one wheel notch scrolls.
    pub const WHEEL: usize = 3;

    /// Steps a scroll's paint takes to catch up with its window: two
    /// ticks, 120 ms at the runtime's tick — long enough for a window
    /// to draw a dozen frames of the ease, short enough that a cell
    /// surface never shows more than one frame on the way.
    pub const GLIDE_STEPS: u64 = 2;

    /// A viewport following the tail.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            top: None,
            lag: Eased::spanning(Self::GLIDE_STEPS),
        }
    }

    /// A viewport pinned to the first row instead — what a list whose
    /// news arrives at the top wants (the Roadmap tab's working set),
    /// as against a transcript whose news arrives at the bottom.
    #[must_use]
    pub const fn at_top() -> Self {
        Self {
            top: Some(0),
            lag: Eased::spanning(Self::GLIDE_STEPS),
        }
    }

    /// Whether the window follows the tail.
    #[must_use]
    pub const fn is_following(&self) -> bool {
        self.top.is_none()
    }

    /// The first visible row for `total` rows in `height` rows of screen.
    #[must_use]
    pub fn top(&self, total: usize, height: usize) -> usize {
        let max = total.saturating_sub(height);
        self.top.map_or(max, |top| top.min(max))
    }

    /// The first visible row a cell surface paints at animation step
    /// `now`: the settled [`Viewport::top`] displaced by what the glide
    /// will have left to run *a step from now* — the frame is held for
    /// a whole tick, so it shows where the paint is by the time the
    /// next one lands — rounded to a whole row and held inside the same
    /// clamp. Once the glide is over — and for a viewport nothing ever
    /// scrolled — this is exactly [`Viewport::top`].
    #[must_use]
    pub fn top_at(&self, total: usize, height: usize, now: f32) -> usize {
        let settled = self.top(total, height);
        let lag = self.lag.at(now + 1.0);
        if lag == 0.0 {
            return settled;
        }
        let max = total.saturating_sub(height);
        let row = (settled as f32 + lag).round();
        if row <= 0.0 {
            return 0;
        }
        (row as usize).min(max)
    }

    /// The row the paint is *between* at step `now` — the settled top
    /// displaced by the glide's own value at that instant, fractional,
    /// inside the clamp — for a surface that can paint a fraction of a
    /// row. Once the glide is over this is [`Viewport::top`] exactly.
    #[must_use]
    pub fn top_between(&self, total: usize, height: usize, now: f32) -> f32 {
        let settled = self.top(total, height) as f32;
        let max = total.saturating_sub(height) as f32;
        (settled + self.lag.at(now)).clamp(0.0, max)
    }

    /// Whether the paint is still catching up with the window at step
    /// `now`, so the runtime should keep its tick running.
    #[must_use]
    pub fn is_gliding(&self, now: f32) -> bool {
        self.lag.is_settling(now)
    }

    /// Re-pin the window at `row` — the row the content it was showing
    /// moved to when the turn re-wrapped — leaving the glide in flight
    /// exactly where it was, so the paint moves with its rows. A window
    /// following the tail has nothing to hold and stays following; a
    /// row at or past the last window follows the tail again, the same
    /// rule a scroll that reaches the bottom takes.
    pub fn hold(&mut self, row: usize, total: usize, height: usize) {
        if self.top.is_none() {
            return;
        }
        let max = total.saturating_sub(height);
        self.top = (row < max).then_some(row);
    }

    /// Scroll by `delta` rows (negative is up); reaching the bottom
    /// follows the tail again. The window lands at once; the paint is
    /// left behind it and eases home from step `now`, or lands with it
    /// when `animate` is off.
    pub fn scroll_by(
        &mut self,
        delta: isize,
        total: usize,
        height: usize,
        now: f32,
        animate: bool,
    ) {
        let max = total.saturating_sub(height);
        let was = self.top(total, height);
        let next = was.saturating_add_signed(delta).min(max);
        self.top = (next < max).then_some(next);
        // What the paint has to catch up on: the rows just travelled,
        // plus whatever a glide already in flight had left to go — which
        // is what makes a notch mid-glide start from where the paint is.
        // Capped at this scroll's own travel, and never more than a
        // windowful: a burst of notches on one frame is one glide that
        // leaves the paint at most a notch behind the window it landed
        // on, never a screen behind that snaps when the wheel stops;
        // Home over a long turn flies in from just off screen instead of
        // strobing through every row on the way.
        let cap = delta.unsigned_abs().min(height.max(1)) as f32;
        let travelled = was as f32 - next as f32 + self.lag.at(now);
        self.lag
            .glide_from(travelled.clamp(-cap, cap), 0.0, now, animate);
    }

    /// One wheel notch up.
    pub fn wheel_up(&mut self, total: usize, height: usize, now: f32, animate: bool) {
        self.scroll_by(-(Self::WHEEL as isize), total, height, now, animate);
    }

    /// One wheel notch down.
    pub fn wheel_down(&mut self, total: usize, height: usize, now: f32, animate: bool) {
        self.scroll_by(Self::WHEEL as isize, total, height, now, animate);
    }

    /// A page up (one row of overlap).
    pub fn page_up(&mut self, total: usize, height: usize, now: f32, animate: bool) {
        self.scroll_by(-page(height), total, height, now, animate);
    }

    /// A page down (one row of overlap).
    pub fn page_down(&mut self, total: usize, height: usize, now: f32, animate: bool) {
        self.scroll_by(page(height), total, height, now, animate);
    }

    /// The top of the turn.
    pub fn home(&mut self, total: usize, height: usize, now: f32, animate: bool) {
        self.scroll_by(isize::MIN, total, height, now, animate);
    }

    /// The tail, followed from now on.
    pub fn end(&mut self, total: usize, height: usize, now: f32, animate: bool) {
        self.scroll_by(isize::MAX, total, height, now, animate);
    }
}

fn page(height: usize) -> isize {
    isize::try_from(height.saturating_sub(1).max(1)).unwrap_or(isize::MAX)
}

#[cfg(test)]
#[path = "viewport.tests.rs"]
mod tests;
