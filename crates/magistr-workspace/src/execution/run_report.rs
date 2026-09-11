use serde::Serialize;

use super::RunFailure;

/// What a runner parser extracted (wit: `run-report`): every failure it
/// found and the runner's own one-line summary when it printed one.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RunReport {
    pub failures: Vec<RunFailure>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
}

#[cfg(feature = "sandbox")]
impl From<crate::worker::WorkerRunReport> for RunReport {
    fn from(report: crate::worker::WorkerRunReport) -> Self {
        Self {
            failures: report.failures.into_iter().map(Into::into).collect(),
            summary: report.summary,
        }
    }
}
