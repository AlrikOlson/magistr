use serde::{Deserialize, Serialize};

/// Why a run cannot admit another attempt; none of these certifies a task.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StopReason {
    OneTaskComplete,
    IterationLimit,
    NoProgress,
    BudgetExceeded,
    SpecComplete,
    NoReadyTask,
    Cancelled,
    Failed,
    Aborted,
    Paused,
}

impl StopReason {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::OneTaskComplete => "one_task_complete",
            Self::IterationLimit => "iteration_limit",
            Self::NoProgress => "no_progress",
            Self::BudgetExceeded => "budget_exceeded",
            Self::SpecComplete => "spec_complete",
            Self::NoReadyTask => "no_ready_task",
            Self::Cancelled => "cancelled",
            Self::Failed => "failed",
            Self::Aborted => "aborted",
            Self::Paused => "paused",
        }
    }
}

impl core::fmt::Display for StopReason {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(self.as_str())
    }
}
