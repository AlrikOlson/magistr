use alloc::string::String;

use serde::{Deserialize, Serialize};

use super::check_state::CheckState;
use super::standard_check_kind::StandardCheckKind;

/// Result of running a single check.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StandardCheckOutcome {
    /// Which check was run.
    pub kind: StandardCheckKind,
    /// Final status.
    pub status: CheckState,
    /// Check output (stdout + stderr), if captured.
    pub output: Option<String>,
    /// Duration in milliseconds, if measured.
    pub duration_ms: Option<u64>,
}

impl StandardCheckOutcome {
    /// Whether this check passed.
    pub fn passed(&self) -> bool {
        matches!(self.status, CheckState::Passed | CheckState::Cached)
    }
}
