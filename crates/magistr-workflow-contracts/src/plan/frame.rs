//! Typed mutation records; emission belongs to the workflow host.

use super::Task;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum PlanFrame {
    TaskAdded {
        task: Task,
    },
    /// A non-terminal change (status/update/reprioritize/link).
    TaskChanged {
        task: Task,
    },
    /// Terminal: the task shipped. Triggers a session commit when shared.
    TaskCompleted {
        task: Task,
    },
    /// Terminal: the task was retired. Triggers a session commit when shared.
    TaskRetired {
        task: Task,
    },
    RefreshRecorded {
        summary: String,
        notes: Vec<u32>,
    },
}

impl PlanFrame {
    /// `(kind, closes_session)` for the git-native record mapping.
    /// `kind` is the Agent Trace record discriminator; `closes_session` marks
    /// the task-lifecycle close events that trigger a commit.
    pub fn record_meta(&self) -> (&'static str, bool) {
        match self {
            Self::TaskAdded { .. } | Self::TaskChanged { .. } => ("task", false),
            Self::TaskCompleted { .. } | Self::TaskRetired { .. } => ("task", true),
            Self::RefreshRecorded { .. } => ("refresh", false),
        }
    }
}
