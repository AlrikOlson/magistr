use magistr_domain::identity::AttemptId;

use crate::Revision;

/// Acknowledgement of the named operation, correlated to one command attempt.
/// This is not a turn result, a passing check, a durable commit or a guarantee
/// of exactly-once execution. The concrete use case defines what was acknowledged.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandReceipt {
    pub attempt: AttemptId,
    pub revision: Revision,
}
