//! Phase completion status enum.

use serde::{Deserialize, Serialize};

/// Phase completion status.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PhaseStatus {
    /// Not started.
    #[default]
    ToDo,
    /// Work in progress.
    InProgress,
    /// All tasks complete.
    Done,
    /// Waiting on dependencies or something outside the plan.
    Waiting,
}

impl PhaseStatus {
    /// Human-readable label.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::ToDo => "to_do",
            Self::InProgress => "in_progress",
            Self::Done => "done",
            Self::Waiting => "waiting",
        }
    }
}

impl std::fmt::Display for PhaseStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.label())
    }
}
