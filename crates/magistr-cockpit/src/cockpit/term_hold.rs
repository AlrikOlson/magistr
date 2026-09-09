//! The press-and-hold that kills a running program.

use std::time::Duration;

use super::kill_cmd::KillCmd;
use super::model::Model;

/// A press-and-hold gesture for stopping a running program.
///
/// Holding `k` or the card header for half a second fills the header red and
/// stops the program. Releasing early cancels the gesture. Key and pointer
/// holds are tracked separately so one input's release cannot cancel the other.
///
/// All timing is derived from model ticks. The fill begins after
/// [`TermHold::DEAD`], allowing a short click to fold the card without flashing
/// red. The gesture completes at [`TermHold::HOLD`].
#[derive(Clone, Debug, PartialEq)]
pub struct TermHold {
    target: KillCmd,
    from: f32,
    by_cap: bool,
}

impl TermHold {
    /// The whole hold, in animation steps: the design's half second on
    /// the cockpit's tick.
    pub const HOLD: f32 = Self::steps(Duration::from_millis(500));
    /// The steps a press may last before the head starts to fill — a
    /// click's length, so a fold never flashes red.
    pub const DEAD: f32 = Self::steps(Duration::from_millis(120));
    /// The tolerance on the hold's end: a thousandth of a step.
    const EPSILON: f32 = 1e-3;

    /// `duration` in animation steps at [`Model::TICK_PERIOD`].
    #[allow(
        clippy::cast_precision_loss,
        reason = "a duration of a few hundred milliseconds is exact in f32"
    )]
    const fn steps(duration: Duration) -> f32 {
        duration.as_millis() as f32 / Model::TICK_PERIOD.as_millis() as f32
    }

    /// A hold the key started on `target` at step `now`.
    #[must_use]
    pub const fn key(target: KillCmd, now: f32) -> Self {
        Self {
            target,
            from: now,
            by_cap: false,
        }
    }

    /// A hold a press on the card's head started on `target` at step
    /// `now`.
    #[must_use]
    pub const fn cap(target: KillCmd, now: f32) -> Self {
        Self {
            target,
            from: now,
            by_cap: true,
        }
    }

    /// What the hold would kill.
    #[must_use]
    pub const fn target(&self) -> &KillCmd {
        &self.target
    }

    /// Whether the cap holds it (the button's release cancels), rather
    /// than the key (the key's release cancels).
    #[must_use]
    pub const fn is_by_cap(&self) -> bool {
        self.by_cap
    }

    /// How far the head has filled at step `now`: nothing inside the
    /// dead zone, then the share of the hold that is done, one when
    /// it is.
    #[must_use]
    pub fn fraction(&self, now: f32) -> f32 {
        let held = now - self.from;
        if held <= Self::DEAD {
            return 0.0;
        }
        if self.is_done(now) {
            return 1.0;
        }
        ((held - Self::DEAD) / (Self::HOLD - Self::DEAD)).clamp(0.0, 1.0)
    }

    /// Whether the hold has lasted the whole half second at step `now`
    /// — read with a hair of tolerance, since the step is a float sum
    /// of ticks and phases and the last bit is not a design decision.
    #[must_use]
    pub fn is_done(&self, now: f32) -> bool {
        now - self.from >= Self::HOLD - Self::EPSILON
    }
}

#[cfg(test)]
#[path = "term_hold.tests.rs"]
mod tests;
