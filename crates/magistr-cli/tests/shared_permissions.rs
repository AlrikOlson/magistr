//! Compare real project-policy construction used by session hosts before presentation.
use agent_client_protocol::schema::v1::ToolKind;
use magistr::agent::permission_request::permission_request as policy_request;
use magistr::agent::{AgentConfig, noninteractive_response::noninteractive_response};
use magistr::orchestration::commands::run::cockpit_policy_for_project;
use magistr::permissions::{CharterStore, NoninteractivePolicy, PermissionProfile};
use magistr_acp::{
    AcpPermissionOption, AcpPermissionOptionKind as Kind, AcpPermissionRequest,
    AcpPermissionResponse,
};
use magistr_application::sessions::{PermissionKind, SessionPolicy};
use magistr_engine::policy::{
    Charter, PermissionOutcome, PolicyCondition, PolicyEffect, PolicyRule,
};

fn request(tool: &str, input: serde_json::Value, kind: Option<ToolKind>) -> AcpPermissionRequest {
    AcpPermissionRequest {
        tool_call_id: "call".into(),
        tool_title: tool.into(),
        tool_input: input,
        tool_kind: kind,
        options: vec![
            AcpPermissionOption {
                option_id: "reject".into(),
                kind: Kind::RejectOnce,
            },
            AcpPermissionOption {
                option_id: "allow".into(),
                kind: Kind::AllowOnce,
            },
        ],
        response_tx: tokio::sync::oneshot::channel().0,
    }
}

fn project() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join(".claude")).unwrap();
    std::fs::create_dir_all(dir.path().join(".magistr")).unwrap();
    std::fs::write(
        dir.path().join(".claude/permissions.toml"),
        "profile = \"standard\"\n",
    )
    .unwrap();
    std::fs::write(
        dir.path().join(".magistr-roadmap.yml"),
        "# protected legacy project fixture\n",
    )
    .unwrap();
    std::fs::write(dir.path().join(".magistr/team.json"), serde_json::json!({
        "policy": {"rules": [
            PolicyRule::new("no-publish", PolicyCondition::tool("Publish"), PolicyEffect::Deny),
            PolicyRule::new("review-release", PolicyCondition::tool("Release"), PolicyEffect::RequireApproval)
        ]}
    }).to_string()).unwrap();
    dir
}

#[test]
fn project_policy_decisions_match_between_cockpit_and_agent_session_for_both_run_modes() {
    let dir = project();
    for workflow in [false, true] {
        let config = AgentConfig::new("fixture", dir.path())
            .model("fixture-model")
            .workflow_permissions(workflow);
        let (session, handling) = config.session_permission_policy();
        let mut session = session.audit_to(None);
        let mut cockpit = cockpit_policy_for_project(
            dir.path(),
            PermissionProfile::Standard,
            workflow,
            Some("fixture-model"),
        )
        .audit_to(None);
        assert_eq!(handling, NoninteractivePolicy::Refuse);
        let requests = [
            (
                request(
                    "Read",
                    serde_json::json!({"file_path": "src/main.rs"}),
                    Some(ToolKind::Read),
                ),
                PermissionOutcome::Allow,
            ),
            (
                request(
                    "Write",
                    serde_json::json!({"file_path": ".magistr-roadmap.yml"}),
                    Some(ToolKind::Edit),
                ),
                PermissionOutcome::Deny,
            ),
            (
                request("Publish", serde_json::json!({}), None),
                PermissionOutcome::Deny,
            ),
            (
                request("Release", serde_json::json!({}), None),
                PermissionOutcome::Ask,
            ),
        ];
        for (request, expected) in requests {
            let left = session.evaluate_request(&request);
            let right = cockpit.evaluate_request(&request);
            assert_eq!(
                left, right,
                "same project/context must have identical core decisions"
            );
            assert_eq!(left.outcome, expected);
            assert!(!left.scope.is_empty());
            assert!(!left.reason.is_empty());
            assert_ne!(left.policy_version, 0);
        }
    }
}

#[test]
fn noninteractive_automatic_response_changes_only_ask_presentation_and_never_denial() {
    let dir = project();
    let (policy, _) = AgentConfig::new("fixture", dir.path()).session_permission_policy();
    let mut policy = policy.audit_to(None);
    let ask = request("Release", serde_json::json!({}), None);
    let core = policy.evaluate_request(&ask);
    assert_eq!(core.outcome, PermissionOutcome::Ask);
    assert!(matches!(
        noninteractive_response(&mut policy, &ask, NoninteractivePolicy::Refuse),
        AcpPermissionResponse::Cancelled
    ));
    assert!(
        matches!(noninteractive_response(&mut policy, &ask, NoninteractivePolicy::LegacyAutomaticApproval), AcpPermissionResponse::Selected(id) if id == "allow")
    );
    assert_eq!(
        policy.evaluate_request(&ask),
        core,
        "presentation does not mutate policy"
    );
    let deny = request("Publish", serde_json::json!({}), None);
    for mode in [
        NoninteractivePolicy::Refuse,
        NoninteractivePolicy::LegacyAutomaticApproval,
    ] {
        assert!(
            matches!(noninteractive_response(&mut policy, &deny, mode), AcpPermissionResponse::Selected(id) if id == "reject")
        );
    }
    let mut unavailable = request("Release", serde_json::json!({}), None);
    unavailable
        .options
        .retain(|option| option.kind == Kind::RejectOnce);
    assert!(matches!(
        noninteractive_response(
            &mut policy,
            &unavailable,
            NoninteractivePolicy::LegacyAutomaticApproval
        ),
        AcpPermissionResponse::Cancelled
    ));
}

#[test]
fn remembered_answers_survive_a_surface_change_but_do_not_override_governance() {
    let dir = project();
    let ask = request(
        "Write",
        serde_json::json!({"file_path": "crates/a/src/lib.rs"}),
        Some(ToolKind::Edit),
    );
    let mut cockpit =
        cockpit_policy_for_project(dir.path(), PermissionProfile::Standard, false, None)
            .audit_to(None);
    let before = cockpit.evaluate_request(&ask);
    assert_eq!(before.outcome, PermissionOutcome::Ask);
    cockpit.answered(&policy_request(&ask), PermissionKind::AllowAlways);
    let (session, _) = AgentConfig::new("fixture", dir.path()).session_permission_policy();
    let mut session = session.audit_to(None);
    let after = session.evaluate_request(&ask);
    assert_eq!(after.outcome, PermissionOutcome::Allow);
    assert_ne!(before.policy_version, after.policy_version);
    assert_eq!(after, cockpit.evaluate_request(&ask));
    assert_eq!(
        session
            .evaluate_request(&request(
                "Write",
                serde_json::json!({"file_path": "crates/b/src/lib.rs"}),
                Some(ToolKind::Edit)
            ))
            .outcome,
        PermissionOutcome::Ask
    );

    let mut charter = CharterStore::at(dir.path()).load().unwrap();
    charter.delegate(PolicyRule::new(
        "release-allowed",
        PolicyCondition::tool("Release"),
        PolicyEffect::Allow,
    ));
    CharterStore::at(dir.path()).save(&charter).unwrap();
    let (governed, _) = AgentConfig::new("fixture", dir.path()).session_permission_policy();
    let mut governed = governed.audit_to(None);
    assert_eq!(
        governed
            .evaluate_request(&request("Release", serde_json::json!({}), None))
            .outcome,
        PermissionOutcome::Ask
    );
}

#[test]
fn project_scope_and_policy_changes_do_not_reuse_another_projects_decision() {
    let first = project();
    let second = project();
    let req = request(
        "Write",
        serde_json::json!({"file_path": "src/main.rs"}),
        Some(ToolKind::Edit),
    );
    let (a, _) = AgentConfig::new("fixture", first.path()).session_permission_policy();
    let (b, _) = AgentConfig::new("fixture", second.path()).session_permission_policy();
    let a = a.audit_to(None).evaluate_request(&req);
    let b = b.audit_to(None).evaluate_request(&req);
    assert_eq!(a.outcome, b.outcome);
    assert_ne!(a.scope, b.scope, "decision scope contains project identity");
    let mut charter = Charter::default();
    charter.delegate(PolicyRule::new(
        "refuse-writes",
        PolicyCondition::tool("Write"),
        PolicyEffect::Deny,
    ));
    CharterStore::at(first.path()).save(&charter).unwrap();
    let (changed, _) = AgentConfig::new("fixture", first.path()).session_permission_policy();
    let changed = changed.audit_to(None).evaluate_request(&req);
    assert_eq!(changed.outcome, PermissionOutcome::Deny);
    assert_ne!(a.policy_version, changed.policy_version);
}
