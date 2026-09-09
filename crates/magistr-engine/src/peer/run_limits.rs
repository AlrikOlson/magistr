use serde::{Deserialize, Serialize};

/// Fixed run ceilings. These are committed values, not runtime usage accounting.
/// Time and tokens spent reconciling or gathering evidence share these ceilings.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RunLimits {
    pub deadline_unix_ms: u64,
    pub max_total_tokens: u64,
    /// Total worker starts, including retries. New runs deny additional admissions
    /// at this count while allowing current work to finish within the other limits.
    /// Restored version 1 journals retain their terminal-at-equality semantics.
    pub max_attempts: u32,
    pub max_reconciliation_rounds: u32,
    pub max_retries_per_task: u32,
}

impl RunLimits {
    /// Require finite caller-selected ceilings; default to two reconciliation rounds
    /// and no automatic retries. The host still enforces remaining budget and time.
    #[must_use]
    pub fn new(deadline_unix_ms: u64, max_total_tokens: u64, max_attempts: u32) -> Self {
        Self {
            deadline_unix_ms,
            max_total_tokens,
            max_attempts,
            max_reconciliation_rounds: 2,
            max_retries_per_task: 0,
        }
    }
}
