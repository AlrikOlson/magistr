use serde::Serialize;

/// The retained structured-failure quartet (wit: `structured-failure`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RunFailure {
    pub test_name: String,
    /// Workspace-relative path when the runner's output names one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file: Option<String>,
    /// The assertion or error message, verbatim from the runner.
    pub assertion: String,
    /// The trace, already trimmed to the load-bearing frames.
    pub trace: String,
}

#[cfg(feature = "sandbox")]
impl From<crate::worker::WorkerRunFailure> for RunFailure {
    fn from(failure: crate::worker::WorkerRunFailure) -> Self {
        Self {
            test_name: failure.test_name,
            file: failure.file,
            assertion: failure.assertion,
            trace: failure.trace,
        }
    }
}
