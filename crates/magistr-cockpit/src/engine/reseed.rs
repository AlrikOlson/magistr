//! Re-reading the store: on a timer while no feed is attached, and on the
//! feed's request whenever it may have missed something.

use std::time::Duration;

use magistr_engine::NativeReader;
use tokio::task::JoinHandle;

use super::event::EngineEvent;
use super::feed_link::FeedLink;
use crate::channel::Inbox;
use crate::cockpit::Msg;

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
/// Ends when the loop is gone; abort the handle to end it sooner.
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
/// The read itself is blocking file I/O, so it runs on the blocking pool
/// rather than on the runtime's threads.
pub fn spawn_reseed(
    engines: NativeReader,
    link: FeedLink,
    inbox: Inbox,
    period: Duration,
) -> JoinHandle<()> {
    tokio::spawn(reseed(engines, link, inbox, period))
}

async fn reseed(engines: NativeReader, link: FeedLink, inbox: Inbox, period: Duration) {
    // What the entry point's own snapshot already covered.
    let mut seen = engines.snapshot_cursor();
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
        let Ok(Some((cursor, snapshot))) = read else {
            continue;
        };
        seen = cursor;
        if !inbox.send(Msg::Engine(EngineEvent::Seed(snapshot))) {
            return;
        }
    }
}

#[cfg(test)]
#[path = "reseed.tests.rs"]
mod tests;
