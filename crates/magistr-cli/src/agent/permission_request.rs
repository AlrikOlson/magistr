//! Translate provider permission data without passing its response channel to policy.

use magistr_acp::{AcpPermissionOptionKind, AcpPermissionRequest};
use magistr_application::sessions::{PermissionKind, SessionPolicyRequest};

/// Preserve the complete policy input independently of bounded observations.
#[must_use]
pub fn permission_request(request: &AcpPermissionRequest) -> SessionPolicyRequest {
    SessionPolicyRequest {
        tool_call_id: request.tool_call_id.clone(),
        tool_title: request.tool_title.clone(),
        input_json: request.tool_input.to_string(),
        tool_kind: request.tool_kind.map(super::translate::tool_verb),
    }
}

pub(crate) fn permission_kind(kind: AcpPermissionOptionKind) -> PermissionKind {
    match kind {
        AcpPermissionOptionKind::AllowOnce => PermissionKind::AllowOnce,
        AcpPermissionOptionKind::AllowAlways => PermissionKind::AllowAlways,
        AcpPermissionOptionKind::RejectOnce => PermissionKind::RejectOnce,
        AcpPermissionOptionKind::RejectAlways => PermissionKind::RejectAlways,
    }
}
