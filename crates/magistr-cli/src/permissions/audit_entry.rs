//! A single audit log entry.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use super::decision_reason::DecisionReason;
use super::profile_action::ProfileAction;
use super::risk_level::RiskLevel;

/// A single audit log entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditEntry {
    /// When the decision was made.
    pub timestamp: DateTime<Utc>,
    /// Which tool was evaluated.
    pub tool_name: String,
    /// Risk level of the tool call.
    pub risk: RiskLevel,
    /// Whether it was allowed or denied.
    pub allowed: bool,
    /// Why the decision was made.
    pub reason: String,
    /// Project context (if available).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub project: Option<String>,
    /// Brief summary of the tool input.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input_summary: Option<String>,
    /// Who acted — the resolved team identity id (self-asserted
    /// attribution, not authentication). Absent in older logs.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub actor: Option<String>,
}

impl AuditEntry {
    /// Create an entry from a permission decision and user choice.
    pub fn from_decision(
        tool_name: &str,
        risk: RiskLevel,
        action: ProfileAction,
        reason: &DecisionReason,
        project: Option<&str>,
        input_summary: Option<&str>,
        actor: Option<&str>,
    ) -> Self {
        Self {
            timestamp: Utc::now(),
            tool_name: tool_name.to_string(),
            risk,
            allowed: action == ProfileAction::Allow,
            reason: reason.to_string(),
            project: project.map(String::from),
            input_summary: input_summary.map(String::from),
            actor: actor.map(String::from),
        }
    }
}
