//! One thing on the cockpit that moves, and how often it wants a step.

use std::time::Duration;

/// A consumer of the runtime's animation tick, named together with the
/// period it wants steps at. [`Model::animations`](super::Model::animations)
/// declares the *set* that is live right now and the runtime reconciles
/// that set to one interval — the minimum over its members, see
/// [`Animations::period`](super::Animations::period) — so no consumer
/// has to know what else on screen is moving.
///
/// There are deliberately only two. Everything that reads
/// [`Model::ticks`](super::Model::ticks) as if one tick were
/// [`Animation::Motion`]'s period — the spinner, the eased fills, the
/// scroll glides, every transient in the motion layer — is [`Motion`],
/// so whenever any of them is alive the reconciled interval is that
/// period exactly and none of them can be stretched by a coarser
/// declaration. The clocks are [`Clock`]: they read the wall clock the
/// runtime samples per frame (and the loop's own elapsed message), never
/// the tick count, so a second between their steps distorts nothing.
///
/// [`Motion`]: Animation::Motion
/// [`Clock`]: Animation::Clock
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Animation {
    /// Everything derived from [`Model::ticks`](super::Model::ticks):
    /// the spinners, the streaming shimmer, the eased fills, the scroll
    /// glides and every transient in the motion layer.
    Motion,
    /// The two clocks — the header's `mm:ss` since the cockpit started
    /// and the loop status's elapsed. A second is all they can show.
    Clock,
}

impl Animation {
    /// Every animation, in period order (finest first) — what
    /// [`Animations`](super::Animations) folds over.
    pub const ALL: [Self; 2] = [Self::Motion, Self::Clock];

    /// How long one step of this animation lasts.
    ///
    /// [`Animation::Motion`] is fast enough for an eased fill to read as
    /// a glide (about sixteen steps a second) and slow enough that a
    /// frame per step costs nothing against the input-to-paint bound;
    /// [`Animation::Clock`] is the coarsest thing that still shows every
    /// second the clocks count.
    #[must_use]
    pub const fn period(self) -> Duration {
        match self {
            Self::Motion => Duration::from_millis(60),
            Self::Clock => Duration::from_secs(1),
        }
    }

    /// This animation's place in the set's bitmask.
    pub(super) const fn bit(self) -> u8 {
        1 << (self as u8)
    }
}

#[cfg(test)]
#[path = "animation.tests.rs"]
mod tests;
