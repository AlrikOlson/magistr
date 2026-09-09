use magistr_domain::identity::DecisionId;

use super::PermissionAnswer;

/// Answer one decision scoped to its requesting turn, session, run and project.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PermissionCommand {
    pub decision: DecisionId,
    pub answer: PermissionAnswer,
}
