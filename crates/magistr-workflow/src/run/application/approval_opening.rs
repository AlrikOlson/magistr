use serde::Serialize;

use crate::run::approval::Approval;

/// The result of opening a approval, including the existing headless fallback.
#[derive(Debug, Clone, Serialize)]
pub struct ApprovalOpening {
    opened: bool,
    state: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    approval_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    question: Option<String>,
    #[serde(rename = "default", skip_serializing_if = "Option::is_none")]
    default_key: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    expires_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    resolved: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    reason: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    hint: Option<&'static str>,
}

impl ApprovalOpening {
    pub(crate) fn opened(approval: Approval) -> Self {
        Self {
            opened: true,
            state: "pending",
            approval_id: Some(approval.id),
            question: Some(approval.question),
            default_key: Some(approval.default_key),
            expires_at: Some(approval.expires_at),
            resolved: None,
            reason: None,
            hint: Some(
                "poll approval_wait(approval_id) until it returns answered or expired; an unanswered approval resolves to its default at expires_at",
            ),
        }
    }

    pub(crate) fn unopened(default: String, reason: String) -> Self {
        Self {
            opened: false,
            state: "unopened",
            approval_id: None,
            question: None,
            default_key: None,
            expires_at: None,
            resolved: Some(default),
            reason: Some(reason),
            hint: None,
        }
    }
}
