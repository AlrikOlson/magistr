use super::SearchHit;
use crate::worker::WorkerTestRange;

/// Uncapped per-file input to the pure six-stage ranking pipeline.
#[derive(Debug, Clone)]
pub(super) struct RankingCandidate {
    pub file: String,
    pub junk: bool,
    pub test: bool,
    pub hits: Vec<SearchHit>,
    pub test_ranges: Vec<WorkerTestRange>,
}
