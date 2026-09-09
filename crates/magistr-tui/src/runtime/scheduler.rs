//! Deciding when the next frame may be painted.

use std::time::Duration;

use tokio::time::Instant;

use super::activity::Activity;
use super::tier::Tier;

/// The frame policy, pure and clock-free: it remembers whether the model
/// changed since the last paint (`dirty`) and when input and messages
/// last arrived, and answers the [`Tier`] for a given instant.
///
/// Input within [`FrameScheduler::INTERACTIVE_FOR`] means
/// [`Tier::Interactive`]; otherwise a message within
/// [`FrameScheduler::STREAMING_FOR`] means [`Tier::Streaming`]; otherwise
/// [`Tier::Idle`].
///
/// It also owns the animation: [`FrameScheduler::animate`] declares a
/// period a widget wants steps at (the header's spinner while the turn
/// streams), [`FrameScheduler::still`] takes it back. The runtime owns
/// ONE tokio interval, at [`FrameScheduler::period`] — the faster of the
/// tier's budget and the animation's — and runs it only while
/// [`FrameScheduler::wants_frame`], so an idle cockpit with nothing
/// animating paints nothing at all. On each firing
/// [`FrameScheduler::tick_due`] says whether an animation step is owed,
/// so the step and the frame that shows it are the same event and never
/// drift apart.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FrameScheduler {
    dirty: bool,
    last_input: Option<Instant>,
    last_message: Option<Instant>,
    frames: u64,
    animate: Option<Duration>,
    next_tick: Option<Instant>,
}

impl FrameScheduler {
    /// How long after the last input the cockpit stays interactive.
    pub const INTERACTIVE_FOR: Duration = Duration::from_millis(500);
    /// How long after the last message the cockpit stays streaming.
    pub const STREAMING_FOR: Duration = Duration::from_secs(1);
    /// The shortest animation period the runtime will honour: a tokio
    /// interval panics on a zero period, and nothing on a cell grid is
    /// worth more than a thousand steps a second.
    pub const MIN_PERIOD: Duration = Duration::from_millis(1);

    /// A scheduler that wants a first frame and has seen nothing.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            dirty: true,
            last_input: None,
            last_message: None,
            frames: 0,
            animate: None,
            next_tick: None,
        }
    }

    /// Something changed: paint at the next tick.
    pub const fn request(&mut self) {
        self.dirty = true;
    }

    /// Whether a frame is wanted.
    #[must_use]
    pub const fn is_dirty(&self) -> bool {
        self.dirty
    }

    /// Whether the runtime's interval has any reason to run: a frame is
    /// pending, or something is animating and will want one. With
    /// neither, the loop parks on its channels and the cockpit is free.
    #[must_use]
    pub const fn wants_frame(&self) -> bool {
        self.dirty || self.animate.is_some()
    }

    /// Something on screen animates: it wants a step every `period`,
    /// from `now`. Clamped to [`FrameScheduler::MIN_PERIOD`].
    ///
    /// Re-declaring the period already in force is deliberately a no-op:
    /// the core repeats its declaration on every message while the turn
    /// streams, and rescheduling there would keep resetting the spinner
    /// just short of its next frame.
    pub fn animate(&mut self, period: Duration, now: Instant) {
        let period = period.max(Self::MIN_PERIOD);
        if self.animate != Some(period) {
            self.animate = Some(period);
            self.next_tick = Some(now + period);
        }
    }

    /// Nothing animates any more.
    pub const fn still(&mut self) {
        self.animate = None;
        self.next_tick = None;
    }

    /// Whether anything animates.
    #[must_use]
    pub const fn is_animating(&self) -> bool {
        self.animate.is_some()
    }

    /// Whether an animation step is owed at `now` — and if so, books the
    /// next one a period after the one just taken, so a late firing
    /// costs one step rather than shifting every step after it.
    pub fn tick_due(&mut self, now: Instant) -> bool {
        let (Some(period), Some(due)) = (self.animate, self.next_tick) else {
            return false;
        };
        if now < due {
            return false;
        }
        self.next_tick = Some(due + period);
        true
    }

    /// How long the runtime's interval waits between firings at `now`:
    /// the tier's frame budget, or the animation's period when that is
    /// shorter.
    #[must_use]
    pub fn period(&self, now: Instant) -> Duration {
        let tier = self.tier(now).period();
        match self.animate {
            Some(period) => tier.min(period),
            None => tier,
        }
    }

    /// Record `activity` at `now`; the model changed with it.
    pub fn note(&mut self, activity: Activity, now: Instant) {
        match activity {
            Activity::Input => self.last_input = Some(now),
            Activity::Message => self.last_message = Some(now),
        }
        self.dirty = true;
    }

    /// The tier in force at `now`.
    #[must_use]
    pub fn tier(&self, now: Instant) -> Tier {
        let within = |at: Option<Instant>, window: Duration| {
            at.is_some_and(|at| now.saturating_duration_since(at) < window)
        };
        if within(self.last_input, Self::INTERACTIVE_FOR) {
            Tier::Interactive
        } else if within(self.last_message, Self::STREAMING_FOR) {
            Tier::Streaming
        } else {
            Tier::Idle
        }
    }

    /// A frame was painted: nothing is pending until the next change.
    pub const fn painted(&mut self) {
        self.dirty = false;
        self.frames += 1;
    }

    /// Frames painted so far.
    #[must_use]
    pub const fn frames(&self) -> u64 {
        self.frames
    }
}

impl Default for FrameScheduler {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
#[path = "scheduler.tests.rs"]
mod tests;
