use super::*;
use crate::permissions::PermissionProfile;

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
fn roadmap_requests_preserve_provider_choices_but_cannot_implement_or_export() {
    let mut base = AgentConfig::new("old opening", "/tmp").model("chosen");
    base.workflow_permissions = true;
    base.resume = Some("old-session".into());
    let config = config(&base);
    assert_eq!(config.model.as_deref(), Some("chosen"));
    assert!(config.prompt.is_empty() && config.resume.is_none());
    assert!(config.planning_only && !config.workflow_permissions);
    for tool in [
        "Bash",
        "Write",
        "Edit",
        "edit",
        "files",
        "script",
        "run",
        "roadmap_start_chunk",
        "roadmap_complete_chunk",
        "roadmap_export",
        "ship_start",
        "ship_complete",
        "ship_finalize",
        "magistr_gate",
    ] {
        assert!(
            !config.allowed_tools.iter().any(|allowed| allowed == tool),
            "{tool}"
        );
    }
    assert!(
        config
            .allowed_tools
            .iter()
            .any(|tool| tool == "roadmap_add_chunk")
    );
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
            policy.decide(&request(name, serde_json::json!({}))),
            PolicyDecision::Deny
        );
    }
    assert_eq!(
        policy.decide(&request(
            "mcp__magistr__roadmap_add_chunk",
            serde_json::json!({"title":"Accessibility"})
        )),
        PolicyDecision::Allow
    );
    for (status, answer) in [
        ("pending", PolicyDecision::Allow),
        ("in_progress", PolicyDecision::Deny),
        ("done", PolicyDecision::Deny),
    ] {
        assert_eq!(
            policy.decide(&request(
                "mcp__magistr__roadmap_set_status",
                serde_json::json!({"status":status})
            )),
            answer
        );
    }
}
