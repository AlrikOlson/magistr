//! Where a store snapshot was read from.

use std::time::SystemTime;

/// The store as it stood when a [`NativeSnapshot`](super::NativeSnapshot)
/// was read: the newest modification time and the summed length of the
/// four families' files.
///
/// A cheap "did anyone else touch this?" for the timer that re-reads the
/// store while no feed is attached — compare two cursors and read only
/// when one moved. It is *not* a revision: two writes inside one mtime
/// tick that leave the length unchanged are indistinguishable, and it
/// says nothing about which family changed. It is also not the feed's
/// [`EventCursor`](super::EventCursor) — a publisher's counter and a
/// filesystem stamp have no order between them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SnapshotCursor {
    /// The newest modification time across the store's files.
    pub modified: SystemTime,
    /// The summed length of the store's files, in bytes — carried
    /// because a filesystem with one-second mtime granularity would
    /// otherwise hide a fast rewrite.
    pub length: u64,
}
