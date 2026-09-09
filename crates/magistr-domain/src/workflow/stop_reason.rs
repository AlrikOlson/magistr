use serde::{Deserialize, Serialize};

/// Why a run cannot admit another attempt; none of these certifies a chunk.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StopReason {
    OneChunkComplete,
    IterationLimit,
    NoProgress,
    BudgetExceeded,
    SpecComplete,
    NoReadyChunk,
    Cancelled,
    Failed,
    Aborted,
    Paused,
}

impl StopReason {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::OneChunkComplete => "one_chunk_complete",
            Self::IterationLimit => "iteration_limit",
            Self::NoProgress => "no_progress",
            Self::BudgetExceeded => "budget_exceeded",
            Self::SpecComplete => "spec_complete",
            Self::NoReadyChunk => "no_ready_chunk",
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
