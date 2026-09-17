//! The broadcast socket, decoded at the edge and relayed onto the inbox.

use std::path::PathBuf;

use magistr_engine::FeedStatus;
use tokio::task::JoinHandle;

use super::feed_link::FeedLink;
use magistr_cockpit::channel::Inbox;
use magistr_cockpit::cockpit::Msg;
use magistr_cockpit::engine::EngineEvent;

/// How long to wait before trying the socket again.
#[cfg(unix)]
const RETRY: std::time::Duration = std::time::Duration::from_millis(300);

/// Attach to the NDJSON broadcast socket at `socket` — bound by the
/// agent's `magistr mcp-serve`, so it may not exist yet and may go away
/// — and post what happens on `inbox` as [`Msg::Engine`]: `Waiting`
/// first, `Connected { reconnect }` on each attachment, an `Event` per
/// line the engines emit — decoded here, at the edge, into the typed
/// [`NativeEvent`](magistr_engine::NativeEvent) with the
/// [`Delivery`](magistr_engine::Delivery) its cursor earns against the
/// ones before it — `Disconnected` when the server hangs up, then another
/// try. Lines this build cannot decode are dropped. Ends when the loop is
/// gone; abort the handle to end it sooner.
///
/// `link` carries what the re-seed needs without going through the model:
/// the attachment bit it stands down on, and a snapshot request on every
/// attachment (frames emitted before this client registered are gone),
/// on a gap in the sequence and on a new owner — the cases where what the
/// cockpit holds may be incomplete and only the store can say. The cursor
/// is kept across reconnects so the gap a hangup opened is seen as one.
///
/// The socket is a Unix domain socket. Where there are none (Windows)
/// the feed posts `Waiting` once and stays that way until the loop is
/// gone — the same absent-feed mode a bind failure leaves behind, so
/// the cockpit runs on its periodic re-seed alone.
pub fn spawn_feed(socket: PathBuf, inbox: Inbox, link: FeedLink) -> JoinHandle<()> {
    tokio::spawn(feed(socket, inbox, link))
}

#[cfg(not(unix))]
async fn feed(_socket: PathBuf, inbox: Inbox, _link: FeedLink) {
    if post(&inbox, FeedStatus::Waiting) {
        inbox.closed().await;
    }
}

#[cfg(unix)]
async fn feed(socket: PathBuf, inbox: Inbox, link: FeedLink) {
    use magistr_engine::FeedCursor;
    use tokio::net::UnixStream;

    let mut attached_before = false;
    let mut cursor = FeedCursor::new();
    if !post(&inbox, FeedStatus::Waiting) {
        return;
    }
    loop {
        if let Ok(stream) = UnixStream::connect(&socket).await {
            let status = FeedStatus::Connected {
                reconnect: attached_before,
            };
            attached_before = true;
            link.set(true);
            // Whatever was emitted before this client was registered is
            // gone; the store has it.
            link.request_snapshot();
            let held = post(&inbox, status) && relay(stream, &inbox, &link, &mut cursor).await;
            link.set(false);
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

/// Relay events until the server hangs up (`true`) or the loop is gone
/// (`false`).
#[cfg(unix)]
async fn relay(
    stream: tokio::net::UnixStream,
    inbox: &Inbox,
    link: &FeedLink,
    cursor: &mut magistr_engine::FeedCursor,
) -> bool {
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
        let Some((event, at)) = NativeFrame::parse_line(&line).and_then(NativeFrame::decode) else {
            continue;
        };
        let delivery = cursor.accept(at);
        if delivery.needs_snapshot() {
            link.request_snapshot();
        }
        if !inbox.send(Msg::Engine(EngineEvent::Event { event, delivery })) {
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
