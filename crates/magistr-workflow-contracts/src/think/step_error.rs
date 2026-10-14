//! Native reasoning refusal with the persistence owner's exact effect.
use crate::{WriteError, WriteErrorKind};

#[derive(Debug, Clone, serde::Serialize)]
pub struct StepError {
    pub error: String,
    pub status: &'static str,
    pub hint: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub store: Option<WriteError>,
    /// An attempted append can be located by this identity after an uncertain write.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub record_id: Option<String>,
}

impl StepError {
    pub fn rejected(message: impl Into<String>, hint: impl Into<String>) -> Self {
        Self {
            error: message.into(),
            status: "failed",
            hint: hint.into(),
            store: None,
            record_id: None,
        }
    }

    pub fn from_write(error: WriteError) -> Self {
        let hint = if matches!(error.kind, WriteErrorKind::Uncertain) {
            "Read the reasoning history before retrying; the attempted change may have been saved."
        } else {
            "Restore access to the reasoning store, then retry the change."
        };
        Self {
            error: error.to_string(),
            status: "failed",
            hint: hint.into(),
            store: Some(error),
            record_id: None,
        }
    }

    pub fn effect(&self) -> &'static str {
        self.store.as_ref().map_or("none", WriteError::effect)
    }

    pub fn kind(&self) -> &'static str {
        match self.store.as_ref().map(|error| &error.kind) {
            Some(WriteErrorKind::Read(_)) => "read_refused",
            Some(WriteErrorKind::BeforeCommit) => "write_failed",
            Some(WriteErrorKind::Uncertain) => "write_uncertain",
            None => "step_rejected",
        }
    }
}

impl From<String> for StepError {
    fn from(message: String) -> Self {
        Self::rejected(message, "Correct the request and try again.")
    }
}

impl std::fmt::Display for StepError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.error)
    }
}

impl std::error::Error for StepError {}
