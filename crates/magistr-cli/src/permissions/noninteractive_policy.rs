//! How a session without an approval interface answers a policy Ask.

use serde::{Deserialize, Serialize};

/// Noninteractive handling never overrides a project policy denial.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum NoninteractivePolicy {
    /// Cancel an unanswered approval request.
    #[default]
    Refuse,
    /// Automatically select AllowOnce for policy Ask; standing grants stay interactive.
    LegacyAutomaticApproval,
}
