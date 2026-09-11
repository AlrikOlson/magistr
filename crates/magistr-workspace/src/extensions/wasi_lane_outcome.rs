use crate::worker::WorkerGuestEnd;

/// One `wasm32-wasip1` command run as the worker answered it: the guest's
/// whole world was argv plus these captured streams.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WasiLaneOutcome {
    pub end: WorkerGuestEnd,
    pub fuel_used: u64,
    pub exit_code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
}
