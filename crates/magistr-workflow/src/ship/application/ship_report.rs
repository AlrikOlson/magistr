use serde::{Deserialize, Serialize};

use crate::ship::domain::artifact::Artifact;

/// Ordinary completion report. Failed required checks are warnings here —
/// completion of a roadmap chunk is where evidence is enforced — and they
/// are told apart: a check that failed and later passed under the same
/// name in this cycle is `recovered`, one whose latest record still fails
/// is `outstanding`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShipReport {
    pub status: String,
    pub summary: Option<String>,
    tasks: TaskCounts,
    pub artifacts_count: usize,
    pub ship_artifacts: Vec<Artifact>,
    pub warnings: Vec<String>,
    #[serde(default)]
    pub recovered_failures: Vec<String>,
    #[serde(default)]
    pub outstanding_failures: Vec<String>,
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
            recovered_failures: Vec::new(),
            outstanding_failures: Vec::new(),
        }
    }

    /// Attach the failure split: what a later pass recovered and what
    /// still stands.
    pub(crate) fn with_failures(
        mut self,
        recovered_failures: Vec<String>,
        outstanding_failures: Vec<String>,
    ) -> Self {
        self.recovered_failures = recovered_failures;
        self.outstanding_failures = outstanding_failures;
        self
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct TaskCounts {
    total: usize,
    completed: usize,
}
