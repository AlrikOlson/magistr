//! Best-effort, fire-and-forget broadcast of execution-trace mutations.
//!
//! Wire format: one newline-delimited JSON object per mutation, with a
//! `family: "ship"` discriminator flattened on top of the typed
//! [`BroadcastFrame`] payload. The actual socket and fan-out tasks live
//! in [`crate::infra::broadcast`]; this module is the typed view the
//! execution engine emits through.

use std::path::PathBuf;

use tracing::warn;

use crate::infra::{Broadcaster as EngineBroadcaster, Family};

pub use magistr_workflow_contracts::ship::ShipFrame as BroadcastFrame;

/// Cheaply cloneable handle that wraps the shared [`engine::Broadcaster`](crate::infra::Broadcaster)
/// and tags every emitted frame with `family: "ship"`.
#[derive(Clone)]
pub struct Broadcaster {
    inner: EngineBroadcaster,
}

impl Broadcaster {
    pub fn spawn(path: PathBuf) -> Option<Self> {
        EngineBroadcaster::spawn(path).map(|inner| Self { inner })
    }

    /// Wrap an existing [`EngineBroadcaster`] (already spawned at the
    /// shared socket) so this family can emit through it without binding
    /// a second listener.
    pub fn from_engine(inner: EngineBroadcaster) -> Self {
        Self { inner }
    }

    pub fn emit(&self, frame: BroadcastFrame) {
        if let Err(e) = self.inner.emit(Family::Ship, &frame) {
            warn!(
                target: "magistr_workflow::ship::broadcast",
                "dropping broadcast frame: {e}",
            );
        }
    }
}
