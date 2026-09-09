use serde::{Deserialize, Serialize};

/// Commands that can be sent to an engine session.
///
/// Commands are validated against the current session state before execution.
/// Invalid commands are rejected with an `EngineError::InvalidCommand`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SessionCommand {
    /// Send a follow-up message in the current conversation.
    SendFollowup { content: String },

    /// Approve a permission request.
    ApprovePermission { request_id: String },

    /// Deny a permission request.
    DenyPermission { request_id: String },

    /// Gracefully cancel the current operation.
    Cancel,

    /// Immediately abort the session (kill the agent process).
    Abort,

    /// Update a config option.
    SetConfig { key: String, value: String },
}

impl SessionCommand {
    /// Human-readable name for this command variant.
    pub fn name(&self) -> &'static str {
        match self {
            Self::SendFollowup { .. } => "SendFollowup",
            Self::ApprovePermission { .. } => "ApprovePermission",
            Self::DenyPermission { .. } => "DenyPermission",
            Self::Cancel => "Cancel",
            Self::Abort => "Abort",
            Self::SetConfig { .. } => "SetConfig",
        }
    }
}

impl std::fmt::Display for SessionCommand {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.name())
    }
}
