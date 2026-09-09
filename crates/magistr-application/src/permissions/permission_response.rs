use magistr_domain::identity::DecisionId;

use crate::CommandReceipt;

use super::PermissionOutcome;

/// Command and decision correlation retained in a permission observation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PermissionResponse {
    pub receipt: CommandReceipt,
    pub decision: DecisionId,
    pub outcome: PermissionOutcome,
}
