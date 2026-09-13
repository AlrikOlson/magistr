//! The blocking effects a session relay owns, off its own task.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use tokio::task::JoinHandle;

use crate::channel::Inbox;
use crate::cockpit::Msg;

/// The port calls a session relay carries out for the cockpit — a
/// workbench read or search, an engine mutation, a checkpoint — each on
/// a blocking thread, so a slow one never stalls the task that observes
/// the provider and forwards its cancellation.
///
/// Three rules make the jobs *owned* rather than merely spawned. A
/// capacity ([`SessionJobs::CAPACITY`]) bounds how many run at once; one
/// more is refused with the reason, for the caller to say on the stage.
/// A fence closes before the relay terminates its session, so a result
/// that lands late posts nothing — the cockpit never sees an answer from
/// a session that is gone. And the relay waits for its jobs under a
/// grace ([`SessionJobs::JOIN_GRACE`]) before it ends, saying how many it
/// had to leave behind; a blocking thread cannot be aborted, only fenced.
pub struct SessionJobs {
    fence: Arc<AtomicBool>,
    running: Vec<JoinHandle<()>>,
}

impl Default for SessionJobs {
    fn default() -> Self {
        Self::new()
    }
}

impl SessionJobs {
    /// Jobs in flight at once, at most.
    pub const CAPACITY: usize = 4;

    /// How long the relay waits for its jobs when it ends.
    pub const JOIN_GRACE: Duration = Duration::from_secs(2);

    /// No jobs, the fence open.
    #[must_use]
    pub fn new() -> Self {
        Self {
            fence: Arc::new(AtomicBool::new(true)),
            running: Vec::new(),
        }
    }

    /// Run `job` on a blocking thread and post its answer on `inbox` —
    /// unless the fence has closed by then. `what` names the ask for the
    /// refusal when the capacity is reached.
    ///
    /// # Errors
    /// The reason the job was not started: the capacity, or a closed
    /// fence.
    pub fn run(
        &mut self,
        what: &str,
        inbox: &Inbox,
        job: impl FnOnce() -> Msg + Send + 'static,
    ) -> Result<(), String> {
        self.running.retain(|handle| !handle.is_finished());
        if !self.fence.load(Ordering::Acquire) {
            return Err(format!("{what}: the session is closing"));
        }
        if self.running.len() >= Self::CAPACITY {
            return Err(format!(
                "{what}: {} jobs are already running for this session; try again when one has answered",
                self.running.len()
            ));
        }
        let fence = Arc::clone(&self.fence);
        let inbox = inbox.clone();
        self.running.push(tokio::task::spawn_blocking(move || {
            let msg = job();
            if fence.load(Ordering::Acquire) {
                inbox.send(msg);
            }
        }));
        Ok(())
    }

    /// Jobs still running.
    #[must_use]
    pub fn in_flight(&mut self) -> usize {
        self.running.retain(|handle| !handle.is_finished());
        self.running.len()
    }

    /// Close the fence — nothing posts from here on — and wait for the
    /// jobs under [`SessionJobs::JOIN_GRACE`]. Answers how many were still
    /// running when the grace ran out.
    pub async fn settle(mut self) -> usize {
        self.fence.store(false, Ordering::Release);
        let deadline = tokio::time::Instant::now() + Self::JOIN_GRACE;
        let mut left = 0;
        for handle in self.running.drain(..) {
            if tokio::time::timeout_at(deadline, handle).await.is_err() {
                left += 1;
            }
        }
        left
    }
}
