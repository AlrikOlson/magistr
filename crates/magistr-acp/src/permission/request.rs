//! A permission request forwarded from the agent to the application.

use agent_client_protocol::schema::v1::ToolKind;

use super::{AcpPermissionOption, AcpPermissionResponse};

/// A permission request forwarded from the ACP agent to the application layer.
///
/// Carries what the agent wants to do and a oneshot for the decision. Dropping
/// the sender without answering makes the transport **deny** — at once for a
/// dropped sender, at its timeout for one held and never written — so the
/// agent is never left hanging on a prompt nobody rendered, and nothing the
/// user did not answer runs on their behalf.
pub struct AcpPermissionRequest {
    /// Tool call id from the protocol.
    pub tool_call_id: String,
    /// Human-readable tool title.
    pub tool_title: String,
    /// Raw tool input.
    pub tool_input: serde_json::Value,
    /// Tool kind (read, edit, execute, …) when the agent said.
    pub tool_kind: Option<ToolKind>,
    /// The choices the agent offers.
    pub options: Vec<AcpPermissionOption>,
    /// Send the decision back here.
    pub response_tx: tokio::sync::oneshot::Sender<AcpPermissionResponse>,
}
