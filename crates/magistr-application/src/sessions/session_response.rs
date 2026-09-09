use crate::CommandReceipt;

use super::SessionOutcome;

/// Correlated command admission evidence and its session observation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionResponse {
    pub receipt: CommandReceipt,
    pub outcome: SessionOutcome,
}
