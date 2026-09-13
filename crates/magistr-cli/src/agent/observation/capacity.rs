//! The explicit bounds one observation channel runs under.

/// How much an observer may fall behind before the channel acts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ObservationCapacity {
    /// Pending lossless items admitted before observation ends with an
    /// overflow marker.
    pub lossless_items: usize,
    /// Pending coalesced text bytes kept before the oldest are trimmed
    /// behind a lag marker.
    pub text_bytes: usize,
}

impl ObservationCapacity {
    /// The production bound: a few thousand decisions and tool events,
    /// and a quarter megabyte of streamed text, waiting at most.
    pub const DEFAULT: Self = Self {
        lossless_items: 4096,
        text_bytes: 256 * 1024,
    };
}

impl Default for ObservationCapacity {
    fn default() -> Self {
        Self::DEFAULT
    }
}
