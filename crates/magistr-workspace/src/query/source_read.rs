use super::source_observation::SourceObservation;

/// The outcome of one bounded live read: the bytes, or the fact that the
/// file is over the live-read ceiling. Callers scanning many files exclude an
/// oversized one by name and keep going; single-file callers turn it into the
/// named refusal.
pub(super) enum SourceRead {
    Observed(SourceObservation),
    /// The file is larger than the ceiling; `bytes` is its observed size.
    Oversized {
        bytes: u64,
    },
}
