use serde::{Deserialize, Serialize};

/// One structured failure lifted from the runner surface.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkerRunFailure {
    pub test_name: String,
    pub file: Option<String>,
    pub assertion: String,
    pub trace: String,
}

impl From<crate::sandbox::LiftedFailure> for WorkerRunFailure {
    fn from(failure: crate::sandbox::LiftedFailure) -> Self {
        Self {
            test_name: failure.test_name,
            file: failure.file,
            assertion: failure.assertion,
            trace: failure.trace,
        }
    }
}
