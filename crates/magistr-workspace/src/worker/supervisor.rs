use super::{
    WorkerError, WorkerIncarnation, WorkerOptions, WorkerRequest, WorkerResponse, WorkerRestore,
    WorkerStatus, frame::Frame, process::WorkerProcess, request_id::next_request_id,
};
use std::{
    collections::VecDeque,
    path::PathBuf,
    sync::{Arc, PoisonError, RwLock, atomic::AtomicU64},
    time::Duration,
};
use tokio::{
    sync::{Mutex, Semaphore},
    time::Instant,
};
use tokio_util::sync::CancellationToken;

/// Bounded application capability over an owned native child, with no MCP calls.
/// Await `shutdown()` before stopping the async runtime to guarantee reaping.
/// Drop is a best-effort guard and depends on that runtime remaining alive.
pub struct WorkerSupervisor {
    inner: Arc<Inner>,
}
struct Inner {
    executable: PathBuf,
    options: WorkerOptions,
    permits: Arc<Semaphore>,
    stopped: CancellationToken,
    next_id: AtomicU64,
    /// Replayed into every incarnation between its readiness frame and its
    /// first dispatched request.
    restore: RwLock<Option<Arc<dyn WorkerRestore>>>,
    state: Mutex<State>,
}
#[derive(Default)]
struct State {
    process: Option<WorkerProcess>,
    starts: u64,
    failures: u32,
    strikes: VecDeque<(String, u32)>,
}

impl WorkerSupervisor {
    pub fn new(executable: PathBuf, options: WorkerOptions) -> Result<Self, WorkerError> {
        if options.max_pending == 0
            || options.max_pending > 1024
            || options.max_strikes == 0
            || options.max_quarantine_entries == 0
            || options.max_quarantine_entries > 65536
            || options.backoff.is_empty()
            || options.ready_timeout.is_zero()
        {
            return Err(WorkerError::Protocol("invalid worker limits".into()));
        }
        Ok(Self {
            inner: Arc::new(Inner {
                executable,
                permits: Arc::new(Semaphore::new(options.max_pending)),
                options,
                stopped: CancellationToken::new(),
                next_id: AtomicU64::new(1),
                restore: RwLock::new(None),
                state: Mutex::new(State::default()),
            }),
        })
    }

    /// Install what every incarnation must replay before it serves. The
    /// composition installs this before the first request can spawn a
    /// process; a process already admitted is not revisited.
    pub fn install_restore(&self, restore: Arc<dyn WorkerRestore>) {
        *self
            .inner
            .restore
            .write()
            .unwrap_or_else(PoisonError::into_inner) = Some(restore);
    }

    pub fn pending(&self) -> usize {
        self.inner.options.max_pending - self.inner.permits.available_permits()
    }

    pub async fn call(
        &self,
        request: WorkerRequest,
        timeout: Duration,
        cancel: &CancellationToken,
    ) -> Result<WorkerResponse, WorkerError> {
        if self.inner.stopped.is_cancelled() {
            return Err(WorkerError::Stopped);
        }
        if cancel.is_cancelled() {
            return Err(WorkerError::Cancelled);
        }
        let deadline = Instant::now()
            .checked_add(timeout)
            .ok_or(WorkerError::Deadline)?;
        let permit = self
            .inner
            .permits
            .clone()
            .try_acquire_owned()
            .map_err(|_| WorkerError::Busy)?;
        // Bound local payloads before queueing or hashing their full contents.
        let id = next_request_id(&self.inner.next_id)?;
        let frame = Frame::Request {
            v: Frame::version(),
            id,
            request,
        };
        let bytes = frame.encode()?;
        let key = match &frame {
            Frame::Request { request, .. } => request.input_key(),
            _ => None,
        };
        drop(frame);
        let cancellation = cancel.child_token();
        // Dropping a caller future cancels its independent cleanup task.
        let _guard = cancellation.clone().drop_guard();
        let inner = self.inner.clone();
        tokio::spawn(async move {
            let _permit = permit;
            inner.execute(bytes, key, id, deadline, &cancellation).await
        })
        .await
        .map_err(|e| WorkerError::Unavailable(e.to_string()))?
    }

    pub async fn shutdown(&self) -> Result<(), WorkerError> {
        self.inner.stopped.cancel();
        // Every admitted task owns one permit until its kill/wait has completed.
        let _all = self
            .inner
            .permits
            .clone()
            .acquire_many_owned(self.inner.options.max_pending as u32)
            .await
            .map_err(|_| WorkerError::Stopped)?;
        let mut state = self.inner.state.lock().await;
        if let Some(mut process) = state.process.take() {
            process.graceful_stop().await?;
        }
        Ok(())
    }

    pub async fn status(&self) -> WorkerStatus {
        let state = self.inner.state.lock().await;
        WorkerStatus {
            pid: state.process.as_ref().and_then(WorkerProcess::pid),
            restarts: state.starts.saturating_sub(1),
            consecutive_failures: state.failures,
            quarantined: state
                .strikes
                .iter()
                .filter(|(_, strikes)| *strikes >= self.inner.options.max_strikes)
                .count(),
            stopped: self.inner.stopped.is_cancelled(),
            pending: self.pending(),
        }
    }
}

impl Drop for WorkerSupervisor {
    fn drop(&mut self) {
        self.inner.stopped.cancel();
        // Active tasks finish their own cleanup. With an idle child, keep its
        // state alive until explicit kill/wait finishes on this runtime.
        if let Ok(runtime) = tokio::runtime::Handle::try_current() {
            let inner = self.inner.clone();
            runtime.spawn(async move {
                let mut state = inner.state.lock().await;
                if let Some(mut process) = state.process.take() {
                    let _ = process.stop().await;
                }
            });
        }
    }
}

impl Inner {
    async fn execute(
        &self,
        bytes: Vec<u8>,
        key: Option<String>,
        id: u64,
        deadline: Instant,
        cancel: &CancellationToken,
    ) -> Result<WorkerResponse, WorkerError> {
        let mut state = tokio::select! {
            biased;
            _ = self.stopped.cancelled() => return Err(WorkerError::Stopped),
            _ = cancel.cancelled() => return Err(WorkerError::Cancelled),
            _ = tokio::time::sleep_until(deadline) => return Err(WorkerError::Deadline),
            state = self.state.lock() => state,
        };
        if let Some(key) = &key
            && state
                .strikes
                .iter()
                .any(|(k, n)| k == key && *n >= self.options.max_strikes)
        {
            return Err(WorkerError::Quarantined(key.clone()));
        }
        let mut dispatched = false;
        let result = tokio::select! {
            biased;
            _ = self.stopped.cancelled() => Err(WorkerError::Stopped),
            _ = cancel.cancelled() => Err(WorkerError::Cancelled),
            _ = tokio::time::sleep_until(deadline) => Err(WorkerError::Deadline),
            result = async {
                self.ensure_ready(&mut state).await?;
                let process = state.process.as_mut().ok_or_else(|| WorkerError::Unavailable("worker did not start".into()))?;
                dispatched = true;
                process.request(&bytes, id).await
            } => result,
        };
        match &result {
            Ok(_) | Err(WorkerError::Remote { .. }) => state.failures = 0,
            Err(error) => {
                if let Some(mut process) = state.process.take() {
                    process.stop().await?;
                }
                let crashed = matches!(
                    error,
                    WorkerError::Unavailable(_)
                        | WorkerError::Protocol(_)
                        | WorkerError::UnsupportedVersion
                );
                if crashed {
                    state.failures = state.failures.saturating_add(1);
                    if dispatched && let Some(key) = key {
                        self.strike(&mut state, key);
                    }
                }
            }
        }
        result
    }

    /// Spawn a replacement when none is admitted. The spawned process stays
    /// local until its readiness frame and the installed restore have both
    /// succeeded: only then is it the process requests are dispatched to,
    /// so an earlier incarnation's readiness never certifies this one. A
    /// caller whose deadline or cancellation interrupts admission drops the
    /// unadmitted process, which its guard kills.
    async fn ensure_ready(&self, state: &mut State) -> Result<(), WorkerError> {
        if state.process.is_some() {
            return Ok(());
        }
        let index = (state.failures as usize).min(self.options.backoff.len() - 1);
        tokio::time::sleep(self.options.backoff[index]).await;
        let mut process = WorkerProcess::spawn(&self.executable, &self.options)?;
        state.starts += 1;
        if let Err(error) = self.admit(&mut process).await {
            process.stop().await?;
            return Err(error);
        }
        state.process = Some(process);
        Ok(())
    }

    async fn admit(&self, process: &mut WorkerProcess) -> Result<(), WorkerError> {
        match tokio::time::timeout(self.options.ready_timeout, Frame::read(&mut process.stdout))
            .await
        {
            Ok(Ok(Frame::Ready { .. })) => {}
            Ok(Ok(_)) => return Err(WorkerError::Protocol("expected readiness frame".into())),
            Ok(Err(error)) => return Err(error),
            Err(_) => {
                return Err(WorkerError::Unavailable(
                    "worker readiness deadline exceeded".into(),
                ));
            }
        }
        let restore = self
            .restore
            .read()
            .unwrap_or_else(PoisonError::into_inner)
            .clone();
        if let Some(restore) = restore {
            let mut incarnation = WorkerIncarnation::new(process, &self.next_id);
            restore.restore(&mut incarnation).await?;
        }
        Ok(())
    }

    fn strike(&self, state: &mut State, key: String) {
        if let Some((_, n)) = state.strikes.iter_mut().find(|(k, _)| *k == key) {
            *n = n.saturating_add(1);
        } else {
            if state.strikes.len() == self.options.max_quarantine_entries {
                state.strikes.pop_front();
            }
            state.strikes.push_back((key, 1));
        }
    }
}
