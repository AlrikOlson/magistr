//! Result of evaluating permission for a tool call.

use super::decision_reason::DecisionReason;
use super::profile_action::ProfileAction;
use super::risk_level::RiskLevel;

/// Result of evaluating permission for a tool call.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PermissionDecision {
    /// The tool that was evaluated.
    pub tool_name: String,
    /// Risk level of the tool call.
    pub risk: RiskLevel,
    /// What action to take.
    pub action: ProfileAction,
    /// Why this decision was made.
    pub reason: DecisionReason,
}
