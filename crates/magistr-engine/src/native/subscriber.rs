use std::path::PathBuf;

use tokio::task::JoinHandle;

use crate::handle::EngineHandle;

/// Connect to a think-and-ship broadcast socket and re-emit every frame it
/// carries as [`crate::handle::EngineEvent::Native`] on `handle`.
///
/// The socket is the external server's (bound only when it runs with
/// `THINK_AND_SHIP_BROADCAST_PATH`); this side is a passive subscriber. A
/// connect failure or a closed socket ends the task quietly — the engines
/// keep working, the live feed is simply absent, exactly the degraded mode
/// the library documents for a bind failure. Roadmap frames additionally
/// refresh the native roadmap progress (see
/// [`EngineHandle::emit_native_frame`]).
pub fn spawn_broadcast_subscriber(path: PathBuf, handle: EngineHandle) -> JoinHandle<()> {
    tokio::spawn(subscribe(path, handle))
}

#[cfg(unix)]
async fn subscribe(path: PathBuf, handle: EngineHandle) {
    use tokio::io::{AsyncBufReadExt, BufReader};
    use tokio::net::UnixStream;

    use super::broadcast_frame::NativeFrame;

    let stream = match UnixStream::connect(&path).await {
        Ok(s) => s,
        Err(e) => {
            tracing::warn!(
                target: "magistr_engine::native",
                "broadcast socket {} unavailable: {e}; running without the live feed",
                path.display()
            );
            return;
        }
    };
    tracing::info!(target: "magistr_engine::native", "subscribed to {}", path.display());
    let mut lines = BufReader::new(stream).lines();
    loop {
        match lines.next_line().await {
            Ok(Some(line)) => {
                if let Some(frame) = NativeFrame::parse_line(&line) {
                    handle.emit_native_frame(frame);
                }
            }
            Ok(None) => break,
            Err(e) => {
                tracing::warn!(target: "magistr_engine::native", "broadcast read failed: {e}");
                break;
            }
        }
    }
    tracing::info!(target: "magistr_engine::native", "broadcast socket {} closed", path.display());
}

/// Unix sockets do not exist here; the feed is absent, as it is on a bind
/// failure elsewhere.
#[cfg(not(unix))]
async fn subscribe(path: PathBuf, _handle: EngineHandle) {
    tracing::info!(
        target: "magistr_engine::native",
        "broadcast socket {} unavailable on this platform",
        path.display()
    );
}

#[cfg(all(test, unix))]
#[path = "subscriber.tests.rs"]
mod tests;
