//! Best-effort, fire-and-forget broadcast of reasoning-trace mutations.
//!
//! Wire format: one newline-delimited JSON object per mutation, with a
//! `family: "think"` discriminator flattened on top of the typed
//! [`BroadcastFrame`] payload. The actual socket and fan-out tasks live
//! in [`crate::infra::broadcast`]; this module is the typed view that
//! the reasoning engine emits through.
//!
//! Emission is never allowed to fail the calling MCP tool path:
//!
//! - Bind failure at startup returns `None` from [`Broadcaster::spawn`];
//!   [`ReasoningServer`] then runs without a broadcaster.
//! - Encode or channel-send failure is logged at `WARN` and discarded.
//!
//! [`ReasoningServer`]: crate::think::engine::core::ReasoningServer

use std::path::PathBuf;

use tracing::warn;

use crate::infra::{Broadcaster as EngineBroadcaster, Family};

/// One mutation event. Encoded as a single JSON object with an externally
/// tagged `type` discriminant so a stream of mixed frames is unambiguous.
/// The wrapping [`Broadcaster`] flattens a `family: "think"` field onto
/// every emitted frame so a shared viewer can interleave events from
/// other tool families on one timeline.
pub use magistr_workflow_contracts::think::ThinkFrame as BroadcastFrame;

/// Cheaply cloneable handle that wraps the shared [`engine::Broadcaster`](crate::infra::Broadcaster)
/// and tags every emitted frame with `family: "think"`.
#[derive(Clone)]
pub struct Broadcaster {
    inner: EngineBroadcaster,
}

impl Broadcaster {
    /// Bind the shared socket at `path` (or join an existing one) and
    /// return a think-family handle. Returns `None` if no tokio runtime
    /// is active or the bind failed; in either case the server runs
    /// unaffected.
    pub fn spawn(path: PathBuf) -> Option<Self> {
        EngineBroadcaster::spawn(path).map(|inner| Self { inner })
    }

    /// Wrap an existing [`EngineBroadcaster`] (already spawned at the
    /// shared socket) so this family can emit through it without binding
    /// a second listener.
    pub fn from_engine(inner: EngineBroadcaster) -> Self {
        Self { inner }
    }

    /// Enqueue a frame for fan-out. Fire-and-forget: any encode or
    /// channel error is logged and dropped so the calling tool path is
    /// never affected.
    pub fn emit(&self, frame: BroadcastFrame) {
        if let Err(e) = self.inner.emit(Family::Think, &frame) {
            warn!(
                target: "magistr_workflow::think::broadcast",
                "dropping broadcast frame: {e}",
            );
        }
    }

    /// Clients the shared accept loop has registered. Connecting is not
    /// the same as being registered — see
    /// [`crate::infra::Broadcaster::subscriber_count`].
    #[cfg(unix)]
    #[must_use]
    pub fn subscriber_count(&self) -> usize {
        self.inner.subscriber_count()
    }

    /// Resolve once at least `n` clients are registered, or `false` on
    /// timeout. Forwards to
    /// [`crate::infra::Broadcaster::wait_for_subscribers`].
    #[cfg(unix)]
    pub async fn wait_for_subscribers(&self, n: usize, timeout: std::time::Duration) -> bool {
        self.inner.wait_for_subscribers(n, timeout).await
    }
}
