//! The window's clock: when the next animation step is owed, and where
//! inside the current step the frame about to be drawn sits.

use std::time::{Duration, Instant};

/// Interpolate rendering between the cockpit's fixed model ticks.
///
/// [`Cadence::owed`] counts model steps due at the current time; [`Cadence::phase`]
/// supplies the fractional position within one step through
/// [`Msg::Phase`](magistr_cockpit::cockpit::Msg::Phase). A display can therefore render
/// smooth intermediate frames without changing model durations or event timestamps.
#[derive(Clone, Copy, Debug)]
pub struct Cadence {
    /// When the next step falls due; `None` while nothing animates.
    next: Option<Instant>,
    /// One frame of the display the window is on.
    refresh: Duration,
}

impl Cadence {
    /// The most steps one wake-up hands over before the clock simply
    /// resynchronises.
    ///
    /// A window that was occluded, dragged between displays or suspended
    /// comes back owing however many steps the absence lasted, and
    /// folding all of them would replay minutes of motion into one
    /// frame. Four is a quarter of a second at the cockpit's tick: long
    /// enough that an ordinary slow frame is caught up rather than
    /// dropped, short enough that nothing visible is replayed.
    pub const CATCH_UP: u32 = 4;

    /// The frame interval assumed until the platform names the
    /// display's own — 60 Hz, the slowest panel anyone ships, so a
    /// window on an unidentified display animates at least as smoothly
    /// as the tick it replaces and never faster than it can be seen.
    pub const DEFAULT_REFRESH: Duration = Duration::from_micros(16_667);

    /// A clock nothing has asked for a step from yet.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            next: None,
            refresh: Self::DEFAULT_REFRESH,
        }
    }

    /// Take the refresh rate the platform reports for the display the
    /// window opened on; `None` leaves the default.
    ///
    /// The loop is paced here rather than by the swapchain because
    /// qs-gpu prefers `Mailbox` wherever the adapter offers it, and
    /// `Mailbox` does not block: a loop that simply asked for a redraw
    /// every pass would render as fast as the GPU can and throw most of
    /// it away. Naming the display's own interval draws each frame once.
    pub fn set_refresh(&mut self, hz: Option<f64>) {
        if let Some(hz) = hz.filter(|hz| *hz > 1.0) {
            self.refresh = Duration::from_secs_f64(1.0 / hz);
        }
    }

    /// When the frame after one painted at `last` falls due.
    #[must_use]
    pub fn frame(&self, last: Instant) -> Instant {
        last + self.refresh
    }

    /// The steps owed at `now`, the clock advanced past them.
    ///
    /// The first call on a stopped clock owes nothing and starts the
    /// count — a cockpit that has just begun animating is at step zero
    /// of its first transient, not one step in.
    pub fn owed(&mut self, now: Instant, period: Duration) -> u32 {
        let Some(next) = self.next else {
            self.next = Some(now + period);
            return 0;
        };
        if now < next {
            return 0;
        }
        let behind = now.saturating_duration_since(next);
        let steps = (behind.div_duration_f64(period) as u32) + 1;
        if steps > Self::CATCH_UP {
            self.next = Some(now + period);
            return Self::CATCH_UP;
        }
        self.next = Some(next + period * steps);
        steps
    }

    /// Where inside the current step `now` sits, `0.0..1.0`.
    ///
    /// Zero on a stopped clock, which is what makes a settled frame the
    /// frame the goldens pin: nothing animates, so nothing is sampled
    /// between two steps.
    #[must_use]
    pub fn phase(&self, now: Instant, period: Duration) -> f32 {
        let Some(next) = self.next else {
            return 0.0;
        };
        let left = next.saturating_duration_since(now).as_secs_f32();
        (1.0 - left / period.as_secs_f32()).clamp(0.0, 1.0)
    }

    /// Stop: the cockpit declares no animation, so there is no step to
    /// owe and no phase to be inside of.
    pub const fn stop(&mut self) {
        self.next = None;
    }

    /// When the next step falls due, for a loop that waits rather than
    /// polls.
    #[must_use]
    pub const fn due(&self) -> Option<Instant> {
        self.next
    }
}

impl Default for Cadence {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
#[path = "cadence.tests.rs"]
mod tests;
