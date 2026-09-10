//! Re-reading the store on a timer while no feed is attached.

use std::time::Duration;

use magistr_engine::NativeReader;
use tokio::task::JoinHandle;

use super::attached::FeedAttached;
use super::event::EngineEvent;
use super::seed::StoreSeed;
use crate::channel::Inbox;
use crate::cockpit::Msg;

/// Re-read the store behind `engines` every `period` and post the result
/// on `inbox` as [`EngineEvent::Seed`], so a cockpit opened without the
/// loop driver picks up what another process (the agent's
/// `magistr mcp-serve`) wrote. Ends when the loop is gone; abort the
/// handle to end it sooner.
///
/// Two things it will not do, both load-bearing:
///
/// * It stands down entirely while `attached` reports the broadcast feed
///   holds the socket — those mutations arrive as frames already, and a
///   full seed on top of them is a re-read nobody asked for.
/// * It re-reads only when [`NativeReader::store_stamp`] moved. A seed
///   posted every period would mark the runtime's scheduler dirty every
///   period, and an idle cockpit that paints nothing is a property the
///   runtime is built around; the reload is also not silent (it prints a
///   load banner into the stderr the session holds), so an ungated timer
///   would fill the replay with banners nobody wrote.
///
/// The read itself is blocking file I/O, so it runs on the blocking pool
/// rather than on the runtime's threads.
pub fn spawn_reseed(
    engines: NativeReader,
    attached: FeedAttached,
    inbox: Inbox,
    period: Duration,
) -> JoinHandle<()> {
    tokio::spawn(reseed(engines, attached, inbox, period))
}

async fn reseed(engines: NativeReader, attached: FeedAttached, inbox: Inbox, period: Duration) {
    // What the entry point's own seed already covered.
    let mut seen = engines.store_stamp();
    loop {
        if !pause(&inbox, period).await {
            return;
        }
        if attached.is_attached() {
            continue;
        }
        let read = engines.clone();
        let read = tokio::task::spawn_blocking(move || {
            let stamp = read.store_stamp();
            if stamp == seen {
                return None;
            }
            read.reload_roadmap();
            read.reload_ship();
            read.reload_signal();
            Some((stamp, StoreSeed::read(&read)))
        })
        .await;
        let Ok(Some((stamp, seed))) = read else {
            continue;
        };
        seen = stamp;
        if !inbox.send(Msg::Engine(EngineEvent::Seed(seed))) {
            return;
        }
    }
}

/// One interval; `false` when the loop went away meanwhile.
async fn pause(inbox: &Inbox, period: Duration) -> bool {
    tokio::select! {
        () = inbox.closed() => false,
        () = tokio::time::sleep(period) => true,
    }
}

#[cfg(test)]
#[path = "reseed.tests.rs"]
mod tests;
