//! How often the cockpit may paint.

use std::time::Duration;

/// A frame budget: the loop paints at most once per period, and only when
/// something changed. The tier follows recent activity — see
/// [`FrameScheduler`](super::FrameScheduler).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Tier {
    /// The user is typing or pointing: 60 fps.
    Interactive,
    /// Messages are flowing (a streaming agent, gates): 30 fps.
    Streaming,
    /// Nothing recent: 4 fps, for the clock and animations only.
    Idle,
}

impl Tier {
    /// The shortest time between two frames in this tier.
    #[must_use]
    pub const fn period(self) -> Duration {
        match self {
            Self::Interactive => Duration::from_millis(16),
            Self::Streaming => Duration::from_millis(33),
            Self::Idle => Duration::from_millis(250),
        }
    }
}
