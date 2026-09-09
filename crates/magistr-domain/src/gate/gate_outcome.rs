use alloc::string::String;

use serde::{Deserialize, Serialize};

use super::gate_kind::GateKind;
use super::gate_status::GateStatus;

/// Result of running a single quality gate.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GateOutcome {
    /// Which gate was run.
    pub kind: GateKind,
    /// Final status.
    pub status: GateStatus,
    /// Gate output (stdout + stderr), if captured.
    pub output: Option<String>,
    /// Duration in milliseconds, if measured.
    pub duration_ms: Option<u64>,
}

impl GateOutcome {
    /// Whether this gate passed.
    pub fn passed(&self) -> bool {
        matches!(self.status, GateStatus::Passed | GateStatus::Cached)
    }
}
