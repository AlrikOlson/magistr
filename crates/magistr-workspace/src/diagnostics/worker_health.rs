use serde::Serialize;

use crate::worker::WorkerStatus;

/// The supervised worker as a status row: a snapshot between requests,
/// never a readiness promise. The quarantine count is the in-memory
/// ledger of this worker session.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkerHealth {
    pub pid: Option<u32>,
    pub restarts: u64,
    pub consecutive_failures: u32,
    pub quarantined: usize,
    pub stopped: bool,
    pub pending: usize,
}

impl From<WorkerStatus> for WorkerHealth {
    fn from(status: WorkerStatus) -> Self {
        Self {
            pid: status.pid,
            restarts: status.restarts,
            consecutive_failures: status.consecutive_failures,
            quarantined: status.quarantined,
            stopped: status.stopped,
            pending: status.pending,
        }
    }
}
