//! One session owner orders workflow reads and their publication.

use std::sync::Arc;

use magistr_cockpit::{cockpit::Msg, engine::EngineEvent};
use magistr_cockpit_runtime::channel::Inbox;
use magistr_engine::{NativeReader, NativeSnapshot, ReadError, ReadErrorKind};
use tokio::sync::Mutex;

/// Reads and publication share one lock, so a subscription refresh cannot
/// apply an older successful read after the timer published a later failure.
#[derive(Clone, Debug)]
pub(crate) struct SnapshotReads {
    reader: NativeReader,
    seen: Arc<Mutex<NativeSnapshot>>,
    publication: Arc<std::sync::Mutex<u64>>,
}

impl SnapshotReads {
    pub fn new(reader: NativeReader) -> Self {
        Self {
            reader,
            seen: Arc::default(),
            publication: Arc::default(),
        }
    }

    /// Invalidations and publication share this short synchronous critical section.
    /// The generation describes scheduling only; it is never store provenance.
    pub fn invalidate(&self, apply: impl FnOnce() -> bool) -> bool {
        let mut generation = self
            .publication
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let requested = apply();
        if requested {
            *generation = generation.wrapping_add(1);
        }
        requested
    }

    pub async fn read(&self, inbox: Option<&Inbox>, requested: bool) -> NativeSnapshot {
        let reader = self.reader.clone();
        self.observe(inbox, requested, move || reader.snapshot())
            .await
    }

    async fn observe(
        &self,
        inbox: Option<&Inbox>,
        requested: bool,
        read: impl FnOnce() -> NativeSnapshot + Send + 'static,
    ) -> NativeSnapshot {
        let mut seen = self.seen.lock().await;
        let began = *self
            .publication
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let snapshot = read_with(seen.clone(), read).await;
        let generation = self
            .publication
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if *generation != began {
            // A newer invalidation already requested another read. Suppress this
            // overtaken result; do not advance the last published baseline.
            return snapshot;
        }
        if (requested || snapshot != *seen)
            && let Some(inbox) = inbox
        {
            inbox.send(Msg::Engine(EngineEvent::Seed(snapshot.clone())));
        }
        *seen = snapshot.clone();
        snapshot
    }
}

async fn read_with(
    previous: NativeSnapshot,
    read: impl FnOnce() -> NativeSnapshot + Send + 'static,
) -> NativeSnapshot {
    match tokio::task::spawn_blocking(read).await {
        Ok(snapshot) => snapshot,
        Err(error) => {
            tracing::warn!(%error, "Workflow store read did not finish");
            previous.failed(ReadError::new(
                ReadErrorKind::Interrupted,
                "Work records could not be read. Trying again.",
            ))
        }
    }
}

#[cfg(test)]
#[path = "read_snapshot.tests.rs"]
mod tests;
