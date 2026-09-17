use super::*;
use crate::agent::permission_request::{permission_kind, permission_request};
use magistr_acp::AcpPermissionOptionKind;
use magistr_application::sessions::{PermissionKind, ToolRecord};

#[test]
fn request_translation_keeps_every_kind_and_full_json_without_the_reply_channel() {
    let input = serde_json::json!({
        "paths": ["src/a.rs", "src/é.rs"],
        "url": "https://example.test/private?key=one",
        "nested": [null, false, 42, 1.25, {"value": "é".repeat(ToolRecord::CAP)}]
    });
    for kind in [
        None,
        Some(ToolKind::Read),
        Some(ToolKind::Edit),
        Some(ToolKind::Delete),
        Some(ToolKind::Move),
        Some(ToolKind::Search),
        Some(ToolKind::Execute),
        Some(ToolKind::Think),
        Some(ToolKind::Fetch),
        Some(ToolKind::SwitchMode),
        Some(ToolKind::Other),
    ] {
        let (response_tx, mut response_rx) = tokio::sync::oneshot::channel();
        let original = AcpPermissionRequest {
            tool_call_id: "opaque/call:42".into(),
            tool_title: "Inspect selected files".into(),
            tool_input: input.clone(),
            tool_kind: kind,
            options: Vec::new(),
            response_tx,
        };
        let portable = permission_request(&original);
        assert_eq!(portable.tool_call_id, original.tool_call_id);
        assert!(portable.input_json.len() > ToolRecord::CAP);
        let decoded = RequestData::decode(&portable).unwrap();
        assert_eq!(decoded.tool_title, original.tool_title);
        assert_eq!(decoded.tool_kind, original.tool_kind);
        assert_eq!(decoded.tool_input, input);
        assert!(matches!(
            response_rx.try_recv(),
            Err(tokio::sync::oneshot::error::TryRecvError::Empty)
        ));
    }
}

#[test]
fn malformed_policy_input_is_an_error_instead_of_empty_arguments() {
    let request = SessionPolicyRequest {
        tool_call_id: "call".into(),
        tool_title: "Write".into(),
        input_json: "{broken".into(),
        tool_kind: Some(ToolVerb::Edit),
    };
    assert!(RequestData::decode(&request).is_err());
}

#[test]
fn every_offered_option_keeps_its_permission_meaning() {
    for (wire, kind) in [
        (
            AcpPermissionOptionKind::AllowOnce,
            PermissionKind::AllowOnce,
        ),
        (
            AcpPermissionOptionKind::AllowAlways,
            PermissionKind::AllowAlways,
        ),
        (
            AcpPermissionOptionKind::RejectOnce,
            PermissionKind::RejectOnce,
        ),
        (
            AcpPermissionOptionKind::RejectAlways,
            PermissionKind::RejectAlways,
        ),
    ] {
        assert_eq!(permission_kind(wire), kind);
    }
}
