use crate::worker::{WorkerGuestEnd, WorkerRunReport};

/// A typed runner invoke as the worker answered it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunnerLaneOutcome {
    pub end: WorkerGuestEnd,
    pub fuel_used: u64,
    /// `can-parse`'s answer; false short-circuits without parse-output.
    pub claimed: bool,
    /// `None` with `claimed` means the adapter could not extract structure.
    pub report: Option<WorkerRunReport>,
}
