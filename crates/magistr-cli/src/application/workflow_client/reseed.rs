//! Re-reading the store: on a timer while no feed is attached, and on the
//! feed's request whenever it may have missed something.

use std::time::Duration;

use magistr_engine::{NativeReader, SnapshotCursor};
use tokio::task::JoinHandle;

use super::feed_link::FeedLink;
use magistr_cockpit::cockpit::Msg;
use magistr_cockpit::engine::EngineEvent;
use magistr_cockpit_runtime::channel::Inbox;

/// Why the re-seed woke.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Wake {
    /// Its own interval passed.
    Timer,
    /// The feed asked for a snapshot.
    Requested,
}

/// Re-read the store behind `engines` and post the result on `inbox` as
/// [`EngineEvent::Seed`]: every `period` while no feed is attached, so a
/// cockpit opened without the loop driver picks up what another process
/// (the agent's `magistr mcp-serve`) wrote; and whenever `link` asks —
/// the feed just attached, saw a gap or a new owner — attached or not.
/// `seeded` is where the snapshot the model was seeded with stood: the
/// baseline every re-read is measured against, so that a store the
/// model has not seen is never taken for one it has. Ends when the loop
/// is gone; abort the handle to end it sooner.
///
/// Two things it will not do, both load-bearing:
///
/// * On its own timer it stands down while `link` reports the broadcast
///   feed holds the socket — those mutations arrive as events already,
///   and a full seed on top of them is a re-read nobody asked for. A
///   request from the feed is different: it names a moment the events
///   are known to be incomplete.
/// * It re-reads only when [`NativeReader::snapshot_cursor`] moved. A seed
///   posted every wake would mark the runtime's scheduler dirty every
///   period, and an idle cockpit that paints nothing is a property the
///   runtime is built around. The engines behind the reader observe their
///   own files on every query anyway; the cursor gate is about when a seed
///   is *posted*, and it keeps the timer's cost at a few `stat`s.
///
/// One thing it does only when asked: a request from the feed over an
/// unchanged store is answered with [`EngineEvent::Confirmed`] — the
/// store stands where the last seed left it, which is what the
/// projection the feed marked stale needs to hear to be current again.
/// The timer never confirms: an unchanged store on a timer wake posts
/// nothing, and the idle cockpit stays idle.
///
/// The read itself is blocking file I/O, so it runs on the blocking pool
/// rather than on the runtime's threads.
pub fn spawn_reseed(
    engines: NativeReader,
    link: FeedLink,
    inbox: Inbox,
    period: Duration,
    seeded: Option<SnapshotCursor>,
) -> JoinHandle<()> {
    tokio::spawn(reseed(engines, link, inbox, period, seeded))
}

async fn reseed(
    engines: NativeReader,
    link: FeedLink,
    inbox: Inbox,
    period: Duration,
    seeded: Option<SnapshotCursor>,
) {
    // What the model already holds: the entry point's own snapshot, then
    // each seed posted from here.
    let mut seen = seeded;
    loop {
        let wake = tokio::select! {
            () = inbox.closed() => return,
            () = tokio::time::sleep(period) => Wake::Timer,
            () = link.snapshot_requested() => Wake::Requested,
        };
        if wake == Wake::Timer && link.is_attached() {
            continue;
        }
        let read = engines.clone();
        let read = tokio::task::spawn_blocking(move || {
            let cursor = read.snapshot_cursor();
            if cursor == seen {
                return None;
            }
            read.reload_roadmap();
            read.reload_ship();
            read.reload_signal();
            Some((cursor, read.snapshot()))
        })
        .await;
        let event = match read {
            Ok(Some((cursor, snapshot))) => {
                seen = cursor;
                EngineEvent::Seed(snapshot)
            }
            Ok(None) if wake == Wake::Requested => EngineEvent::Confirmed { cursor: seen },
            _ => continue,
        };
        if !inbox.send(Msg::Engine(event)) {
            return;
        }
    }
}

#[cfg(test)]
#[path = "reseed.tests.rs"]
mod tests;
