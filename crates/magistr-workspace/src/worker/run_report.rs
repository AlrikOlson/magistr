use serde::{Deserialize, Serialize};

use super::WorkerRunFailure;

/// One lifted run-report crossing the process boundary (wit/runner.wit).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkerRunReport {
    pub failures: Vec<WorkerRunFailure>,
    pub summary: Option<String>,
}

impl From<crate::sandbox::LiftedRunReport> for WorkerRunReport {
    fn from(report: crate::sandbox::LiftedRunReport) -> Self {
        Self {
            failures: report.failures.into_iter().map(Into::into).collect(),
            summary: report.summary,
        }
    }
}
