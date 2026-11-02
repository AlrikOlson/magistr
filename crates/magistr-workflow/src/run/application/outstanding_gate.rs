use serde::{Deserialize, Serialize};

/// A gate a completion cannot accept, and the one reason why: missing,
/// failing, stale against the source, the acceptance or the configured
/// command, self-reported, on another step, or never settled.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OutstandingGate {
    pub gate: String,
    pub check_result_id: Option<String>,
    pub reason: String,
}
