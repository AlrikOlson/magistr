use serde::{Deserialize, Serialize};

/// Backend-agnostic session lifecycle state.
///
/// Mirrors the ACP lifecycle but is protocol-independent, allowing the
/// domain model to evolve separately from the transport layer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionStatus {
    /// Session created but no prompt sent yet.
    Created,
    /// Agent is processing (thinking, executing tools).
    InProgress,
    /// Agent completed a turn and awaits the next prompt.
    Awaiting,
    /// Session completed normally.
    Completed,
    /// Session ended due to an error.
    Failed,
    /// Session was cancelled via graceful cancel.
    Cancelled,
}

impl SessionStatus {
    /// Whether the session is in a terminal state (no further commands accepted).
    pub fn is_terminal(self) -> bool {
        matches!(self, Self::Completed | Self::Failed | Self::Cancelled)
    }

    /// Whether the session can accept a follow-up message.
    pub fn can_send_followup(self) -> bool {
        self == Self::Awaiting
    }

    /// Whether the session can be cancelled.
    pub fn can_cancel(self) -> bool {
        matches!(self, Self::InProgress | Self::Awaiting)
    }

    /// The status this one becomes when the session is restored in
    /// another process.
    ///
    /// A status is kept unless it asserts something about a *live*
    /// process, because the process stayed on the old machine.
    /// [`InProgress`] is the only one that does — it means "an agent is
    /// thinking right now", and on the new host none is — so it lands at
    /// [`Awaiting`], the turn boundary, which is exactly what the
    /// restored session now is: nothing in flight, ready for the next
    /// prompt. Every other status is a fact about the past
    /// ([`Created`], the terminal three) or already a boundary
    /// ([`Awaiting`]), and survives the move unchanged.
    ///
    /// This is a property of the *restore*, never of serialization: a
    /// snapshot round-trips with the status it was captured at, because
    /// the document describes the machine it was taken on.
    ///
    /// [`InProgress`]: SessionStatus::InProgress
    /// [`Awaiting`]: SessionStatus::Awaiting
    /// [`Created`]: SessionStatus::Created
    #[must_use]
    pub fn migrates_as(self) -> Self {
        match self {
            Self::InProgress => Self::Awaiting,
            other => other,
        }
    }
}

impl std::fmt::Display for SessionStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let label = match self {
            Self::Created => "Created",
            Self::InProgress => "In Progress",
            Self::Awaiting => "Awaiting",
            Self::Completed => "Completed",
            Self::Failed => "Failed",
            Self::Cancelled => "Cancelled",
        };
        f.write_str(label)
    }
}
