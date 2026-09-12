//! How a frame reached the subscriber, relative to the ones before it.

/// What a [`FeedCursor`](super::FeedCursor) says about one delivered
/// event — the fact a projection needs to decide whether to fold it and
/// whether what it holds is still current.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Delivery {
    /// This process's own mutation, answered by the writer it acted
    /// through: read-your-writes, nothing to sequence.
    Local,
    /// A frame from a publisher that stamped no cursor.
    Unsequenced,
    /// The first sequenced frame this subscriber has seen: the baseline.
    First,
    /// The next frame in the publisher's sequence.
    InOrder,
    /// A frame at or before the sequence already folded: already applied.
    Duplicate,
    /// Frames between the last one folded and this one never arrived.
    Gap {
        /// How many were missed.
        missed: u64,
    },
    /// A frame from a new owner generation: the store may have changed
    /// hands since the last one folded.
    NewEpoch,
}

impl Delivery {
    /// Whether the event was already folded and must not be folded again.
    #[must_use]
    pub const fn is_duplicate(self) -> bool {
        matches!(self, Self::Duplicate)
    }

    /// Whether what the projection holds can no longer be trusted to be
    /// complete: frames were missed, or the owner changed under it.
    #[must_use]
    pub const fn needs_snapshot(self) -> bool {
        matches!(self, Self::Gap { .. } | Self::NewEpoch)
    }
}
