//! The set of animations the cockpit declares, and the one interval it
//! reconciles to.

use std::time::Duration;

use super::animation::Animation;

/// Which [`Animation`]s are live now — a `Copy` bitset, so the model can
/// answer with one by value on every fold.
///
/// The runtime owns a single interval, and [`Animations::period`] is how
/// a set of independent declarations becomes one: the minimum over the
/// members, so every consumer gets steps at least as often as it asked.
/// An empty set is the idle guarantee — no period, no interval, no
/// frames.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Animations(u8);

impl Animations {
    /// Nothing moves.
    pub const NONE: Self = Self(0);

    /// This set with `animation` added.
    #[must_use]
    pub const fn with(self, animation: Animation) -> Self {
        Self(self.0 | animation.bit())
    }

    /// This set with `animation` added when `live`, unchanged otherwise
    /// — the shape a declaration site reads best in.
    #[must_use]
    pub const fn with_if(self, animation: Animation, live: bool) -> Self {
        if live { self.with(animation) } else { self }
    }

    /// Whether `animation` is live.
    #[must_use]
    pub const fn contains(self, animation: Animation) -> bool {
        self.0 & animation.bit() != 0
    }

    /// Whether nothing at all moves.
    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }

    /// The one interval this set reconciles to: the finest period any
    /// member asked for, or `None` when the set is empty and the runtime
    /// should take its tick back.
    #[must_use]
    pub fn period(self) -> Option<Duration> {
        Animation::ALL
            .into_iter()
            .filter(|animation| self.contains(*animation))
            .map(Animation::period)
            .min()
    }
}

#[cfg(test)]
#[path = "animations.tests.rs"]
mod tests;
