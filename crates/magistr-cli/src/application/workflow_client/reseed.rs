//! Periodically validate every workflow owner, and answer coalesced read requests.

use std::time::Duration;

use tokio::task::JoinHandle;

use super::{feed_link::FeedLink, read_snapshot::SnapshotReads};
use magistr_cockpit_runtime::channel::Inbox;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Wake {
    Timer,
    Requested,
}

/// A connected event feed does not prove the files are readable. Every wake
/// validates all four owners, including failed reads whose file stamps stayed
/// unchanged. Timer reads publish only changed observations; a requested read
/// publishes even unchanged records so an invalidated observation can recover.
/// A newer invalidation suppresses an overtaken result and requests another read.
pub fn spawn_reseed(
    reads: SnapshotReads,
    link: FeedLink,
    inbox: Inbox,
    period: Duration,
) -> JoinHandle<()> {
    tokio::spawn(reseed(reads, link, inbox, period))
}

async fn reseed(reads: SnapshotReads, link: FeedLink, inbox: Inbox, period: Duration) {
    loop {
        let wake = tokio::select! {
            () = inbox.closed() => return,
            () = tokio::time::sleep(period) => Wake::Timer,
            () = link.snapshot_requested() => Wake::Requested,
        };
        tokio::select! {
            () = inbox.closed() => return,
            _ = reads.read(Some(&inbox), wake == Wake::Requested) => {},
        }
    }
}

#[cfg(test)]
#[path = "reseed.tests.rs"]
mod tests;
