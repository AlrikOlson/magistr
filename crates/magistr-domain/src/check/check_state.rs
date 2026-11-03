use core::fmt;
use serde::{Deserialize, Serialize};

/// Live status of a check.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CheckState {
    /// Check is queued but not yet running.
    Pending,
    /// Check is currently executing.
    Running,
    /// Check passed.
    Passed,
    /// Check failed.
    Failed,
    /// Check result was served from cache (no re-run needed).
    Cached,
}

impl CheckState {
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

    /// Icon for display.
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

impl fmt::Display for CheckState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}
