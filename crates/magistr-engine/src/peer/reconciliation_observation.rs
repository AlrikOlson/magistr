use serde::{Deserialize, Serialize};

/// Trusted host readings for the WHOLE run, including reconciliation and evidence.
/// Tokens count every model call; attempts count worker starts (not peer messages).
/// The host must reserve call budgets separately and supply nondecreasing readings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReconciliationObservation {
    pub now_unix_ms: u64,
    pub total_tokens: u64,
    pub worker_attempts: u32,
}
