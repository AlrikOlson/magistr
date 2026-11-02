use crate::plan::domain::{WorkstreamFrontier, WorkstreamHistory, WorkstreamHistoryRecords};

/// A read-only inspection, distinct from the lane's saved focus.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum WorkstreamInspection {
    Unset,
    Current {
        frontier: WorkstreamFrontier,
    },
    Closed {
        feature: String,
        history: WorkstreamHistory,
    },
    History {
        feature: String,
        history: WorkstreamHistoryRecords,
    },
    Missing {
        query: String,
    },
    Ambiguous {
        query: String,
        matches: Vec<String>,
    },
}
