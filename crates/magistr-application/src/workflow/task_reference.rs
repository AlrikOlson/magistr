use alloc::string::String;
use magistr_domain::identity::RunId;

use crate::ApplicationError;

/// A native execution task key associated with a run by the owning adapter.
/// Constructing a reference does not certify the task exists or has passed checks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskReference {
    run: RunId,
    key: String,
}

impl TaskReference {
    pub fn new(run: RunId, key: impl Into<String>) -> Result<Self, ApplicationError> {
        let key = key.into();
        super::validate_key(&key)?;
        Ok(Self { run, key })
    }

    pub fn run(&self) -> &RunId {
        &self.run
    }

    pub fn key(&self) -> &str {
        &self.key
    }
}
