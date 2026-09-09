use crate::workflow::TaskReference;
use alloc::string::String;

/// Native recording evidence returned independently from the gate's status.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CheckRecording {
    /// The adapter persisted a check under the requested run and task.
    Recorded {
        task: TaskReference,
        check_key: String,
    },
    /// Execution settled, but native evidence could not be recorded. A passing
    /// process with this result does not satisfy verification requirements.
    Unavailable { reason: String },
}
