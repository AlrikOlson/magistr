//! The actual outcome retained when a session owner settles.
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SessionOutcome {
    Completed,
    Cancelled,
    Failed,
    Stopped,
}

impl SessionOutcome {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Completed => "completed",
            Self::Cancelled => "cancelled",
            Self::Failed => "failed",
            Self::Stopped => "stopped",
        }
    }
}
