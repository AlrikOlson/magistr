use crate::worker::{WorkerGuestEnd, WorkerLocation};

/// A typed resolver invoke as the worker answered it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolverLaneOutcome {
    pub end: WorkerGuestEnd,
    pub fuel_used: u64,
    pub locations: Vec<WorkerLocation>,
    pub sessions_opened: u32,
    pub sessions_reused: u32,
}
