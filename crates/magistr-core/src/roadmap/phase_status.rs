//! Phase completion status enum.

use serde::{Deserialize, Serialize};

/// Phase completion status.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PhaseStatus {
    /// Not started.
    #[default]
    Pending,
    /// Work in progress.
    InProgress,
    /// All tasks complete.
    Done,
    /// Blocked by dependencies or external factors.
    Blocked,
}

impl PhaseStatus {
    /// Human-readable label.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::InProgress => "in_progress",
            Self::Done => "done",
            Self::Blocked => "blocked",
        }
    }
}

impl std::fmt::Display for PhaseStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.label())
    }
}
