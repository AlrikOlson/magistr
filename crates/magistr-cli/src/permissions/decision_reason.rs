//! Why a particular permission decision was made.

use std::fmt;

/// Why a particular permission decision was made.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DecisionReason {
    /// Profile default for this risk level.
    ProfileDefault,
    /// Matched an explicit auto-allow rule.
    AutoAllowRule(String),
    /// Matched a batch-approval pattern (e.g., quality gate command).
    BatchApproval(String),
    /// Tightened by the org's governance policy — the rule's id.
    GovernancePolicy(String),
    /// Decided by a rule the reader wrote into the charter — the
    /// delegation's id.
    Delegation(String),
    /// Explanation supplied by the shared domain policy service.
    PolicyResolution(String),
}

impl fmt::Display for DecisionReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ProfileDefault => write!(f, "profile default"),
            Self::AutoAllowRule(rule) => write!(f, "auto-allow rule: {rule}"),
            Self::BatchApproval(pattern) => write!(f, "batch approval: {pattern}"),
            Self::GovernancePolicy(rule) => write!(f, "policy rule: {rule}"),
            Self::Delegation(rule) => write!(f, "delegated: {rule}"),
            Self::PolicyResolution(reason) => f.write_str(reason),
        }
    }
}
