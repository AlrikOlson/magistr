use crate::plan::domain::{FeatureFrontier, FeatureHistory, FeatureHistoryRecords};

/// A read-only inspection, distinct from the team's saved focus.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum FeatureInspection {
    Unset,
    Current {
        frontier: FeatureFrontier,
    },
    Closed {
        feature: String,
        history: FeatureHistory,
    },
    History {
        feature: String,
        history: FeatureHistoryRecords,
    },
    Missing {
        query: String,
    },
    Ambiguous {
        query: String,
        matches: Vec<String>,
    },
}
