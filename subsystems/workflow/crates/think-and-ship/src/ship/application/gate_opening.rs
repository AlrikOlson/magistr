use serde::Serialize;

use crate::ship::gate::Gate;

/// The result of opening a gate, including the existing headless fallback.
#[derive(Debug, Clone, Serialize)]
pub struct GateOpening {
    opened: bool,
    state: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    gate_id: Option<String>,
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

impl GateOpening {
    pub(crate) fn opened(gate: Gate) -> Self {
        Self {
            opened: true,
            state: "pending",
            gate_id: Some(gate.id),
            question: Some(gate.question),
            default_key: Some(gate.default_key),
            expires_at: Some(gate.expires_at),
            resolved: None,
            reason: None,
            hint: Some(
                "poll ship_gate_wait(gate_id) until it returns answered or expired; an unanswered gate resolves to its default at expires_at",
            ),
        }
    }

    pub(crate) fn unopened(default: String, reason: String) -> Self {
        Self {
            opened: false,
            state: "unopened",
            gate_id: None,
            question: None,
            default_key: None,
            expires_at: None,
            resolved: Some(default),
            reason: Some(reason),
            hint: None,
        }
    }
}
