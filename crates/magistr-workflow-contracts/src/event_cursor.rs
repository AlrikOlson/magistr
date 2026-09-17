//! Where a frame stands in its publisher's sequence.

use super::owner_generation::OwnerGeneration;

/// The two tags a publisher stamps on every frame it emits: which
/// incarnation of the owner published it (its [`OwnerGeneration`], the
/// feed's *epoch*) and the publisher's own counter for the frame (the
/// *sequence*, one per emitted frame across every family on the socket).
///
/// Ordered within one epoch only: sequences of different epochs are
/// different publishers' counters and compare as a change of owner, never
/// as earlier or later. Neither field is a revision of any record in the
/// store — the store keeps per-record stamps of its own, a snapshot has a
/// [`SnapshotCursor`](super::SnapshotCursor), and nothing combines them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct EventCursor {
    /// The publishing owner's generation.
    pub epoch: OwnerGeneration,
    /// The publisher's counter for this frame, starting at one.
    pub sequence: u64,
}
