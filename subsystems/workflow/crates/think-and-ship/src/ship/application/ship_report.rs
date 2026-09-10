use serde::{Deserialize, Serialize};

use crate::ship::domain::artifact::Artifact;

/// Ordinary completion report. Failed checks remain warnings at this stage;
/// source-bound receipt completion policy belongs to the deferred receipt work.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShipReport {
    pub status: String,
    pub summary: Option<String>,
    tasks: TaskCounts,
    pub artifacts_count: usize,
    pub ship_artifacts: Vec<Artifact>,
    pub warnings: Vec<String>,
}

impl ShipReport {
    pub(crate) fn new(
        total: usize,
        completed: usize,
        artifacts_count: usize,
        ship_artifacts: Vec<Artifact>,
        summary: Option<String>,
        warnings: Vec<String>,
    ) -> Self {
        Self {
            status: "shipped".into(),
            summary,
            tasks: TaskCounts { total, completed },
            artifacts_count,
            ship_artifacts,
            warnings,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct TaskCounts {
    total: usize,
    completed: usize,
}
