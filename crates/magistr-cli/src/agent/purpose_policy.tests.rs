use super::*;
use crate::agent::permission_request::permission_request as policy_request;
use crate::permissions::PermissionProfile;

use magistr_acp::AcpPermissionRequest;

fn request(name: &str, input: serde_json::Value) -> AcpPermissionRequest {
    let (response_tx, _) = tokio::sync::oneshot::channel();
    AcpPermissionRequest {
        tool_call_id: "test".into(),
        tool_title: name.into(),
        tool_input: input,
        tool_kind: None,
        options: vec![],
        response_tx,
    }
}

#[test]
fn roadmap_purpose_is_a_ceiling_even_under_yolo() {
    let mut policy = RoadmapPolicy::new(
        SessionPermissionPolicy::new(PermissionProfile::Yolo, vec![], vec![], vec![], true)
            .audit_to(None),
    );
    for name in [
        "mcp__magistr__roadmap_start_chunk",
        "mcp__magistr__ship_start",
        "Bash",
        "Write",
        "mcp__workbench_mcp__edit",
        "mcp__magistr__roadmap_export",
    ] {
        assert_eq!(
            policy.decide(&policy_request(&request(name, serde_json::json!({})))),
            PolicyDecision::Deny
        );
    }
    for name in [
        "mcp__magistr__execute",
        "mcp__magistr__discover",
        "mcp__magistr__read",
    ] {
        assert_eq!(
            policy.decide(&policy_request(&request(name, serde_json::json!({})))),
            PolicyDecision::Allow
        );
    }
    // Each composed effect is checked by the purpose-bound MCP connection.
}
#[test]
fn planning_keeps_the_project_model_policy() {
    let governance = magistr_engine::policy::GovernancePolicy {
        approved_models: vec!["approved".into()],
        ..Default::default()
    };
    let inner =
        SessionPermissionPolicy::new(PermissionProfile::Standard, vec![], vec![], vec![], false)
            .governed_by(governance.compile());
    let mut policy = RoadmapPolicy::new(inner);
    assert!(!policy.can_use_model("unapproved"));
    policy.model_changed(Some("approved".into()));
    assert!(policy.can_use_model("approved"));
}
