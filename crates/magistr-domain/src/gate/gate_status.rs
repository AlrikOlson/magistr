use core::fmt;
use serde::{Deserialize, Serialize};

/// Live status of a quality gate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GateStatus {
    /// Gate is queued but not yet running.
    Pending,
    /// Gate is currently executing.
    Running,
    /// Gate passed.
    Passed,
    /// Gate failed.
    Failed,
    /// Gate result was served from cache (no re-run needed).
    Cached,
}

impl GateStatus {
    /// Human-readable label.
    pub fn label(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Running => "running",
            Self::Passed => "passed",
            Self::Failed => "failed",
            Self::Cached => "cached",
        }
    }

    /// Icon for TUI display.
    pub fn icon(self) -> &'static str {
        match self {
            Self::Pending => "○",
            Self::Running => "●",
            Self::Passed => "✓",
            Self::Failed => "✗",
            Self::Cached => "◎",
        }
    }

    /// Whether this status represents a terminal (completed) state.
    pub fn is_terminal(self) -> bool {
        matches!(self, Self::Passed | Self::Failed | Self::Cached)
    }
}

impl fmt::Display for GateStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}
