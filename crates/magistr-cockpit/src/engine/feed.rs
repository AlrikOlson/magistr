//! The broadcast socket, relayed onto the inbox.

use std::path::PathBuf;

use magistr_engine::FeedStatus;
use tokio::task::JoinHandle;

use super::attached::FeedAttached;
use super::event::EngineEvent;
use crate::channel::Inbox;
use crate::cockpit::Msg;

/// How long to wait before trying the socket again.
#[cfg(unix)]
const RETRY: std::time::Duration = std::time::Duration::from_millis(300);

/// Attach to the NDJSON broadcast socket at `socket` — bound by the
/// agent's `magistr mcp-serve`, so it may not exist yet and may go away
/// — and post what happens on `inbox` as [`Msg::Engine`]: `Waiting`
/// first, `Connected { reconnect }` on each attachment, a `Frame` per
/// line the engines emit, `Disconnected` when the server hangs up, then
/// another try. Ends when the loop is gone; abort the handle to end it
/// sooner.
///
/// `attached` carries the same news to whoever else needs it without
/// going through the model — the periodic re-seed
/// ([`spawn_reseed`](super::spawn_reseed)) stands down while the socket
/// is delivering frames on its own.
///
/// The socket is a Unix domain socket. Where there are none (Windows)
/// the feed posts `Waiting` once and stays that way until the loop is
/// gone — the same absent-feed mode a bind failure leaves behind, so
/// the cockpit runs on its periodic re-seed alone.
pub fn spawn_feed(socket: PathBuf, inbox: Inbox, attached: FeedAttached) -> JoinHandle<()> {
    tokio::spawn(feed(socket, inbox, attached))
}

#[cfg(not(unix))]
async fn feed(_socket: PathBuf, inbox: Inbox, _attached: FeedAttached) {
    if post(&inbox, FeedStatus::Waiting) {
        inbox.closed().await;
    }
}

#[cfg(unix)]
async fn feed(socket: PathBuf, inbox: Inbox, attached: FeedAttached) {
    use tokio::net::UnixStream;

    let mut attached_before = false;
    if !post(&inbox, FeedStatus::Waiting) {
        return;
    }
    loop {
        if let Ok(stream) = UnixStream::connect(&socket).await {
            let status = FeedStatus::Connected {
                reconnect: attached_before,
            };
            attached_before = true;
            attached.set(true);
            let held = post(&inbox, status) && relay(stream, &inbox).await;
            attached.set(false);
            if !held {
                return;
            }
            if !post(&inbox, FeedStatus::Disconnected) {
                return;
            }
        }
        if !pause(&inbox).await {
            return;
        }
    }
}

/// `false` once the loop is gone.
fn post(inbox: &Inbox, status: FeedStatus) -> bool {
    inbox.send(Msg::Engine(EngineEvent::Feed(status)))
}

/// Relay frames until the server hangs up (`true`) or the loop is gone
/// (`false`).
#[cfg(unix)]
async fn relay(stream: tokio::net::UnixStream, inbox: &Inbox) -> bool {
    use magistr_engine::NativeFrame;
    use tokio::io::{AsyncBufReadExt, BufReader};

    let mut lines = BufReader::new(stream).lines();
    loop {
        let next = tokio::select! {
            () = inbox.closed() => return false,
            next = lines.next_line() => next,
        };
        let Ok(Some(line)) = next else {
            return true;
        };
        if let Some(frame) = NativeFrame::parse_line(&line)
            && !inbox.send(Msg::Engine(EngineEvent::Frame(frame)))
        {
            return false;
        }
    }
}

/// One retry interval; `false` when the loop went away meanwhile.
#[cfg(unix)]
async fn pause(inbox: &Inbox) -> bool {
    tokio::select! {
        () = inbox.closed() => false,
        () = tokio::time::sleep(RETRY) => true,
    }
}

#[cfg(test)]
#[path = "feed.tests.rs"]
mod tests;
