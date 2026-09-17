//! Host decoding of policy input. It carries no pending response or runtime handle.

use agent_client_protocol::schema::v1::ToolKind;
use magistr_acp::AcpPermissionRequest;
use magistr_application::sessions::{SessionPolicyRequest, ToolVerb};

pub(super) struct RequestData {
    pub tool_title: String,
    pub tool_input: serde_json::Value,
    pub tool_kind: Option<ToolKind>,
}

impl RequestData {
    pub(super) fn decode(request: &SessionPolicyRequest) -> Result<Self, serde_json::Error> {
        Ok(Self {
            tool_title: request.tool_title.clone(),
            tool_input: serde_json::from_str(&request.input_json)?,
            tool_kind: request.tool_kind.map(|kind| match kind {
                ToolVerb::Read => ToolKind::Read,
                ToolVerb::Edit => ToolKind::Edit,
                ToolVerb::Delete => ToolKind::Delete,
                ToolVerb::Move => ToolKind::Move,
                ToolVerb::Search => ToolKind::Search,
                ToolVerb::Run => ToolKind::Execute,
                ToolVerb::Think => ToolKind::Think,
                ToolVerb::Fetch => ToolKind::Fetch,
                ToolVerb::Mode => ToolKind::SwitchMode,
                ToolVerb::Tool => ToolKind::Other,
            }),
        })
    }
}

#[cfg(test)]
#[path = "request_data.tests.rs"]
mod tests;

impl From<&AcpPermissionRequest> for RequestData {
    fn from(request: &AcpPermissionRequest) -> Self {
        Self {
            tool_title: request.tool_title.clone(),
            tool_input: request.tool_input.clone(),
            tool_kind: request.tool_kind,
        }
    }
}
