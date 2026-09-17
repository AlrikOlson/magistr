//! Spawning work off the loop thread.

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use tokio::task::JoinError;

use crate::channel::Inbox;
use magistr_cockpit::cockpit::Msg;
use magistr_cockpit::job::{JobId, JobOutput};

/// Spawns closures on tokio's blocking pool and posts each result on the
/// runtime's inbox as [`Msg::Job`].
///
/// Cheap to clone; clones number their jobs from one counter. The loop
/// never waits on a job: a job that takes 300 ms costs the cockpit no
/// frame, and a job that panics comes back as [`JobOutput::Failed`]
/// rather than taking anything down.
#[derive(Clone, Debug)]
pub struct Jobs {
    inbox: Inbox,
    next: Arc<AtomicU64>,
}

impl Jobs {
    /// Jobs that post on `inbox`.
    #[must_use]
    pub fn new(inbox: Inbox) -> Self {
        Self {
            inbox,
            next: Arc::new(AtomicU64::new(0)),
        }
    }

    /// Run `f` on the blocking pool; its output arrives as
    /// `Msg::Job { id, output }` once it returns. Must be called from
    /// inside a tokio runtime.
    pub fn spawn_job<F>(&self, f: F) -> JobId
    where
        F: FnOnce() -> JobOutput + Send + 'static,
    {
        let id = JobId::new(self.next.fetch_add(1, Ordering::Relaxed));
        let inbox = self.inbox.clone();
        let work = tokio::task::spawn_blocking(f);
        tokio::spawn(async move {
            let output = work.await.unwrap_or_else(|e| JobOutput::Failed(failure(e)));
            inbox.send(Msg::Job { id, output });
        });
        id
    }

    /// How many jobs were spawned through this handle family.
    #[must_use]
    pub fn spawned(&self) -> u64 {
        self.next.load(Ordering::Relaxed)
    }
}

/// Why a job did not return: its panic message when it panicked.
fn failure(e: JoinError) -> String {
    if !e.is_panic() {
        return "the job was cancelled".to_owned();
    }
    let panic = e.into_panic();
    panic
        .downcast_ref::<&str>()
        .map(|s| (*s).to_owned())
        .or_else(|| panic.downcast_ref::<String>().cloned())
        .unwrap_or_else(|| "the job panicked".to_owned())
}

#[cfg(test)]
#[path = "jobs.tests.rs"]
mod tests;
