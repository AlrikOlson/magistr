//! Workspace discovery and refusal presentation for the shared MCP bridge.

#[cfg(test)]
#[path = "bridge_session.tests.rs"]
mod tests;

use magistr_workspace::{
    contracts::{EnvelopeVersion, ErrorKind, WorkspaceError, WorkspaceResponse},
    response::ResponseAdapter,
};
use rmcp::model::ProtocolVersion;
use serde_json::{Map, Value, json};

use super::capability::Capability;

use crate::mcp_bridge::SessionPolicy;

impl SessionPolicy for Capability {
    fn initialize(&self, requested: ProtocolVersion) -> Option<Value> {
        serde_json::to_value(Self::initialize(&requested)).ok()
    }
    fn listing(&self) -> Option<Value> {
        Some(self.core_listing())
    }
    fn session_meta(&self) -> Option<&'static str> {
        Some("magistr/sessionId")
    }
    fn unavailable_call(
        &self,
        id: &Value,
        meta: Option<&Map<String, Value>>,
        reason: &str,
    ) -> Value {
        unavailable_call(id, meta, reason)
    }
}

/// The retained `ENGINE_UNAVAILABLE` envelope (recovery `retry`) for a tool
/// call that found no owner, in the envelope version the call negotiated.
fn unavailable_call(id: &Value, meta: Option<&Map<String, Value>>, reason: &str) -> Value {
    let version = ResponseAdapter::negotiate_meta(meta).unwrap_or(EnvelopeVersion::LATEST);
    let response: WorkspaceResponse<Value> = WorkspaceResponse::Failure {
        version,
        error: WorkspaceError::new(
            ErrorKind::EngineUnavailable,
            format!("the workspace owner is unavailable; the next call reconnects: {reason}"),
        ),
        notes: Some(vec![
            "effect unknown: read back the affected object or operation receipt before retrying a write; the bridge keeps its session identity".to_owned(),
            format!("owner: {reason}"),
        ]),
    };
    match ResponseAdapter::seal(&response) {
        Ok(sealed) => json!({
            "jsonrpc": "2.0",
            "id": id,
            "result": {
                "content": [{"type": "text", "text": sealed.json()}],
                "structuredContent": sealed.structured(),
                "isError": true,
            },
        }),
        Err(error) => rpc_error(
            id,
            &format!("{reason}; and the refusal could not be sealed: {error}"),
        ),
    }
}

fn rpc_error(id: &Value, reason: &str) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "error": {"code": -32000, "message": format!("workspace owner unavailable: {reason}")},
    })
}
