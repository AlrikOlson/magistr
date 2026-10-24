use serde::{Deserialize, Serialize};

/// How a check's command ended. A timeout, a cancellation and a launch
/// failure are statements about the clock, the caller and the host; only
/// an exit is a statement about the code, and only exit zero passes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum CheckExecution {
    /// The command exited with `code`.
    Exited { code: i32 },
    /// An owner-validated reference to an earlier successful execution.
    Reused { receipt_id: String },
    /// No eligible earlier evidence was available; no command was launched.
    ReuseUnavailable { reason: String },
    /// The command was killed at the caller's deadline.
    TimedOut,
    /// The run was ended from outside before the command finished.
    Cancelled,
    /// The command could not be started at all.
    LaunchFailed { reason: String },
    /// No command ran: the caller reported the verdict itself.
    SelfReported { passed: bool },
}

impl CheckExecution {
    /// Whether this execution counts as a pass.
    #[must_use]
    pub const fn passed(&self) -> bool {
        matches!(
            self,
            Self::Exited { code: 0 } | Self::SelfReported { passed: true } | Self::Reused { .. }
        )
    }

    /// The process exit code, when there was a process that exited.
    #[must_use]
    pub const fn exit_code(&self) -> Option<i32> {
        match self {
            Self::Exited { code } => Some(*code),
            _ => None,
        }
    }

    /// Whether a server ran a command for this outcome.
    #[must_use]
    pub const fn verified(&self) -> bool {
        !matches!(
            self,
            Self::SelfReported { .. } | Self::ReuseUnavailable { .. }
        )
    }

    /// The word for this outcome.
    #[must_use]
    pub fn label(&self) -> String {
        match self {
            Self::Exited { code } => format!("exit {code}"),
            Self::Reused { receipt_id } => format!("reused {receipt_id}"),
            Self::ReuseUnavailable { reason } => format!("not run: {reason}"),
            Self::TimedOut => "timed out".into(),
            Self::Cancelled => "cancelled".into(),
            Self::LaunchFailed { reason } => format!("launch failed: {reason}"),
            Self::SelfReported { passed: true } => "self-reported pass".into(),
            Self::SelfReported { passed: false } => "self-reported failure".into(),
        }
    }
}
