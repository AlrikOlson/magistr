use crate::worker::{WorkerGuestEnd, WorkerRankingFeature};

/// A batched `contribute` pass as the worker answered it.
#[derive(Debug, Clone, PartialEq)]
pub struct RankingContributeOutcome {
    pub end: WorkerGuestEnd,
    pub fuel_used: u64,
    /// Parallel to the input batch; shorter on a mid-batch failure.
    pub contributions: Vec<Vec<WorkerRankingFeature>>,
}
