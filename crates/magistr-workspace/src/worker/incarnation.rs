use std::{sync::atomic::AtomicU64, time::Duration};

use super::{
    WorkerError, WorkerRequest, WorkerResponse, frame::Frame, process::WorkerProcess,
    request_id::next_request_id,
};

/// One freshly spawned, ready worker process during its restore step: the
/// only way to reach a process the supervisor has not admitted yet. Every
/// request made here is answered before any caller's request is dispatched
/// to the same process.
pub struct WorkerIncarnation<'a> {
    process: &'a mut WorkerProcess,
    next_id: &'a AtomicU64,
}

impl<'a> WorkerIncarnation<'a> {
    pub(super) fn new(process: &'a mut WorkerProcess, next_id: &'a AtomicU64) -> Self {
        Self { process, next_id }
    }

    #[must_use]
    pub fn pid(&self) -> Option<u32> {
        self.process.pid()
    }

    /// One request to this incarnation. A lost process or a `timeout` that
    /// expires is `Unavailable`: the supervisor then stops the process
    /// without admitting it, exactly as it treats a crash.
    pub async fn call(
        &mut self,
        request: WorkerRequest,
        timeout: Duration,
    ) -> Result<WorkerResponse, WorkerError> {
        let id = next_request_id(self.next_id)?;
        let bytes = Frame::Request {
            v: Frame::version(),
            id,
            request,
        }
        .encode()?;
        match tokio::time::timeout(timeout, self.process.request(&bytes, id)).await {
            Ok(result) => result,
            Err(_) => Err(WorkerError::Unavailable(
                "worker restore request timed out".into(),
            )),
        }
    }
}
