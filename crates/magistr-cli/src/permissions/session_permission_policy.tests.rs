use super::*;
use crate::agent::permission_request::permission_request as policy_request;
use crate::permissions::CockpitPolicy;
use magistr_engine::policy::RiskTier;

use magistr_acp::AcpPermissionRequest;
use tempfile::TempDir;

/// A request as the transport would forward it; the reply channel is
/// dropped — `decide` never answers, only classifies.
fn request(title: &str, input: serde_json::Value, kind: Option<ToolKind>) -> AcpPermissionRequest {
    let (response_tx, _response_rx) = tokio::sync::oneshot::channel();
    AcpPermissionRequest {
        tool_call_id: "tc-1".to_owned(),
        tool_title: title.to_owned(),
        tool_input: input,
        tool_kind: kind,
        options: Vec::new(),
        response_tx,
    }
}

fn policy(profile: PermissionProfile, workflow: bool) -> CockpitPolicy {
    CockpitPolicy::new(profile, Vec::new(), Vec::new(), Vec::new(), workflow).audit_to(None)
}

#[test]
fn malformed_portable_input_cannot_allow_or_create_a_standing_rule() {
    let mut policy = policy(PermissionProfile::Yolo, true);
    let request = SessionPolicyRequest {
        tool_call_id: "broken".into(),
        tool_title: "Write".into(),
        input_json: "{broken".into(),
        tool_kind: Some(magistr_application::sessions::ToolVerb::Edit),
    };
    assert_eq!(policy.decide(&request), PolicyDecision::Deny);
    policy.answered(&request, PermissionKind::AllowAlways);
    policy.constrain(&request, "allow this");
    assert!(policy.charter().delegations.is_empty());
}

fn audit_entries(path: &std::path::Path) -> Vec<AuditEntry> {
    let content = std::fs::read_to_string(path).unwrap_or_default();
    content
        .lines()
        .map(|line| serde_json::from_str(line).expect("an NDJSON audit entry"))
        .collect()
}

#[test]
fn a_read_the_standard_profile_allows_is_allowed() {
    let mut policy = policy(PermissionProfile::Standard, false);
    let decision = policy.decide(&policy_request(&request(
        "Read",
        serde_json::json!({"file_path": "/src/main.rs"}),
        None,
    )));
    assert_eq!(decision, PolicyDecision::Allow);
}

#[test]
fn a_write_on_the_standard_profile_asks() {
    let mut policy = policy(PermissionProfile::Standard, false);
    let decision = policy.decide(&policy_request(&request(
        "Write",
        serde_json::json!({"file_path": "/src/new.rs"}),
        None,
    )));
    assert_eq!(
        decision,
        PolicyDecision::Ask(RiskTier::Medium),
        "a write is a reversible file change, and the ask carries that"
    );
}

#[test]
fn a_protected_path_write_is_denied() {
    let mut policy = CockpitPolicy::new(
        PermissionProfile::Yolo,
        Vec::new(),
        Vec::new(),
        vec![PathBuf::from("/project/.magistr/roadmap.yaml")],
        false,
    )
    .audit_to(None);
    let decision = policy.decide(&policy_request(&request(
        "Write",
        serde_json::json!({"file_path": "/project/.magistr/roadmap.yaml"}),
        None,
    )));
    assert_eq!(decision, PolicyDecision::Deny, "even yolo never writes it");
}

#[cfg(unix)]
#[test]
fn a_protected_missing_target_is_denied_through_a_symlinked_parent() {
    let dir = tempfile::tempdir().unwrap();
    let actual = dir.path().join("actual/nested");
    std::fs::create_dir_all(&actual).unwrap();
    std::os::unix::fs::symlink(&actual, dir.path().join("alias")).unwrap();
    let root = dir.path().canonicalize().unwrap();
    let mut policy = CockpitPolicy::new(
        PermissionProfile::Yolo,
        Vec::new(),
        Vec::new(),
        vec![root.join("actual/secret.txt")],
        true,
    )
    .rooted_at(Some(root))
    .audit_to(None);
    assert_eq!(
        policy.decide(&policy_request(&write_request("alias/../secret.txt"))),
        PolicyDecision::Deny
    );
}

#[cfg(unix)]
#[test]
fn a_protected_alias_and_a_direct_resolved_target_have_the_same_floor() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir(dir.path().join("actual")).unwrap();
    std::os::unix::fs::symlink("actual", dir.path().join("alias")).unwrap();
    let root = dir.path().canonicalize().unwrap();
    let mut policy = CockpitPolicy::new(
        PermissionProfile::Yolo,
        Vec::new(),
        Vec::new(),
        vec![root.join("alias/not-created.txt")],
        true,
    )
    .rooted_at(Some(root))
    .audit_to(None);
    assert_eq!(
        policy.decide(&policy_request(&write_request("actual/not-created.txt"))),
        PolicyDecision::Deny
    );
}

#[test]
fn process_scope_uses_command_even_when_resolved_cwd_is_a_policy_path() {
    let policy =
        policy(PermissionProfile::Standard, false).rooted_at(Some(PathBuf::from("/project")));
    let request = request(
        "Bash",
        serde_json::json!({
            "command": "sh -c 'printf example'", "executable": "/bin/sh",
            "cwd": "/project/src", "path": "/project/src",
        }),
        Some(ToolKind::Execute),
    );
    assert_eq!(
        policy.scope_of(&RequestData::from(&request)).unwrap().key,
        "bash:sh"
    );
}

#[cfg(unix)]
#[test]
fn unresolvable_symlink_targets_are_denied_even_with_workflow_allowance() {
    let dir = tempfile::tempdir().unwrap();
    std::os::unix::fs::symlink("loop", dir.path().join("loop")).unwrap();
    let mut charter = Charter::default();
    charter.delegate(PolicyRule::new(
        "allow:write",
        magistr_engine::policy::PolicyCondition::tool("Write"),
        PolicyEffect::Allow,
    ));
    let mut policy = policy(PermissionProfile::Yolo, true)
        .rooted_at(Some(dir.path().to_path_buf()))
        .chartered(charter, None);
    let request = write_request("loop/secret.txt");
    let resolution = policy.evaluate_request(&request);
    assert_eq!(resolution.outcome, PermissionOutcome::Deny);
    assert_eq!(resolution.source, PermissionSource::Profile);
    assert!(
        policy.scope_of(&RequestData::from(&request)).is_none(),
        "an unresolved target cannot receive standing authority"
    );
}

#[test]
fn a_relative_spelling_of_a_protected_path_is_denied_once_the_policy_is_rooted() {
    for spelling in [
        "ROADMAP.md",
        "./ROADMAP.md",
        "src/../ROADMAP.md",
        "/project/./ROADMAP.md",
    ] {
        let mut policy = CockpitPolicy::new(
            PermissionProfile::Yolo,
            Vec::new(),
            Vec::new(),
            vec![PathBuf::from("/project/ROADMAP.md")],
            true,
        )
        .rooted_at(Some(PathBuf::from("/project")))
        .audit_to(None);
        assert_eq!(
            policy.decide(&policy_request(&write_request(spelling))),
            PolicyDecision::Deny,
            "{spelling} names the protected file"
        );
    }
}

#[test]
fn a_different_file_with_the_same_name_is_not_blocked() {
    for spelling in ["/elsewhere/ROADMAP.md", "docs/ROADMAP.md", "OLDROADMAP.md"] {
        let mut policy = CockpitPolicy::new(
            PermissionProfile::Yolo,
            Vec::new(),
            Vec::new(),
            vec![PathBuf::from("/project/ROADMAP.md")],
            true,
        )
        .rooted_at(Some(PathBuf::from("/project")))
        .audit_to(None);
        assert_eq!(
            policy.decide(&policy_request(&write_request(spelling))),
            PolicyDecision::Allow,
            "{spelling} is not the protected file"
        );
    }
}

#[test]
fn without_a_root_a_relative_target_matches_by_trailing_components_only() {
    let protect = || {
        CockpitPolicy::new(
            PermissionProfile::Yolo,
            Vec::new(),
            Vec::new(),
            vec![PathBuf::from("/project/.magistr/roadmap.yaml")],
            true,
        )
        .audit_to(None)
    };
    assert_eq!(
        protect().decide(&policy_request(&write_request(".magistr/roadmap.yaml"))),
        PolicyDecision::Deny,
        "the trailing components are the protected entry's"
    );
    assert_eq!(
        protect().decide(&policy_request(&write_request("roadmap.yaml"))),
        PolicyDecision::Deny,
        "so is the bare file name"
    );
    assert_eq!(
        protect().decide(&policy_request(&write_request("old-roadmap.yaml"))),
        PolicyDecision::Allow,
        "a string suffix is not a component"
    );
    assert_eq!(
        protect().decide(&policy_request(&write_request(
            "/other/.magistr/roadmap.yaml"
        ))),
        PolicyDecision::Allow,
        "an absolute path elsewhere is a different file"
    );
}

#[test]
fn a_symlinked_root_still_blocks_the_protected_file() {
    let dir = TempDir::new().expect("a temp dir");
    let protected = dir.path().join("ROADMAP.md");
    std::fs::write(&protected, "").expect("write");
    let canonical = std::fs::canonicalize(&protected).expect("canonical");
    // The entry was recorded canonical; the agent writes through the
    // uncanonical root the session was opened on.
    let mut policy = CockpitPolicy::new(
        PermissionProfile::Yolo,
        Vec::new(),
        Vec::new(),
        vec![canonical],
        true,
    )
    .rooted_at(Some(dir.path().to_path_buf()))
    .audit_to(None);
    assert_eq!(
        policy.decide(&policy_request(&write_request("ROADMAP.md"))),
        PolicyDecision::Deny
    );
}

#[test]
fn a_protected_path_write_is_denied_even_in_workflow_mode() {
    let mut policy = CockpitPolicy::new(
        PermissionProfile::Standard,
        Vec::new(),
        Vec::new(),
        vec![PathBuf::from("/project/.magistr/roadmap.yaml")],
        true,
    )
    .audit_to(None);
    let decision = policy.decide(&policy_request(&request(
        "Edit",
        serde_json::json!({"file_path": "/project/.magistr/roadmap.yaml"}),
        Some(ToolKind::Edit),
    )));
    assert_eq!(
        decision,
        PolicyDecision::Deny,
        "the protected block outranks the workflow auto-approval"
    );
}

#[test]
fn every_path_in_a_multi_file_edit_is_checked_before_workflow_auto_approval() {
    let mut policy = CockpitPolicy::new(
        PermissionProfile::Yolo,
        Vec::new(),
        Vec::new(),
        vec![PathBuf::from("/project/.magistr/roadmap.yaml")],
        true,
    )
    .rooted_at(Some(PathBuf::from("/project")))
    .audit_to(None);
    for input in [
        serde_json::json!({"paths":["src/allowed.rs",".magistr/roadmap.yaml"]}),
        serde_json::json!({"file_path":"src/allowed.rs","paths":["src/another.rs",".magistr/roadmap.yaml"]}),
        serde_json::json!({"file_path":"src/allowed.rs","path":".magistr/roadmap.yaml"}),
        serde_json::json!({"target_file":".magistr/roadmap.yaml"}),
    ] {
        assert_eq!(
            policy.decide(&policy_request(&request(
                "Edit",
                input.clone(),
                Some(ToolKind::Edit)
            ))),
            PolicyDecision::Deny,
            "every target must be protected, regardless of field or position: {input}"
        );
    }
    assert_eq!(
        policy.decide(&policy_request(&request(
            "Edit",
            serde_json::json!({"paths":["src/allowed.rs","src/another.rs"]}),
            Some(ToolKind::Edit)
        ))),
        PolicyDecision::Allow
    );
}

#[test]
fn workflow_mode_auto_approves_read_edit_and_execute_by_kind() {
    for kind in [ToolKind::Read, ToolKind::Edit, ToolKind::Execute] {
        let mut policy = policy(PermissionProfile::Cautious, true);
        let decision = policy.decide(&policy_request(&request(
            "anything",
            serde_json::json!({}),
            Some(kind),
        )));
        assert_eq!(
            decision,
            PolicyDecision::Allow,
            "{kind:?} is a standard development operation in workflow mode"
        );
    }
}

#[test]
fn outside_workflow_an_execute_kind_is_the_profiles_call() {
    let mut policy = policy(PermissionProfile::Cautious, false);
    let decision = policy.decide(&policy_request(&request(
        "Bash",
        serde_json::json!({"command": "rm -rf /tmp/scratch"}),
        Some(ToolKind::Execute),
    )));
    assert_eq!(
        decision,
        PolicyDecision::Ask(RiskTier::High),
        "`rm -rf` is what the checkpoint-before-risk hook keys on"
    );
}

#[test]
fn a_safe_shell_command_is_auto_approved_outside_workflow() {
    let mut policy = policy(PermissionProfile::Cautious, false);
    let decision = policy.decide(&policy_request(&request(
        "Bash",
        serde_json::json!({"command": "cd /project && git status"}),
        Some(ToolKind::Execute),
    )));
    assert_eq!(decision, PolicyDecision::Allow, "safe prefix behind a cd");
}

fn incomplete_shell_inputs() -> [serde_json::Value; 13] {
    [
        serde_json::Value::Null,
        serde_json::json!({}),
        serde_json::json!({"description": "Run the build"}),
        serde_json::json!("git status"),
        serde_json::json!(["git status"]),
        serde_json::json!({"command": null}),
        serde_json::json!({"command": false}),
        serde_json::json!({"command": 0}),
        serde_json::json!({"command": ["git", "status"]}),
        serde_json::json!({"command": {"command": "git status"}}),
        serde_json::json!({"command": ""}),
        serde_json::json!({"command": " \t\r\n"}),
        serde_json::json!({"command": "\u{2003}\u{00a0}"}),
    ]
}

#[test]
fn incomplete_shell_input_cannot_receive_a_safe_command_allowance() {
    for input in incomplete_shell_inputs() {
        assert!(!is_safe_shell_command(&input), "{input}");
    }
}

#[test]
fn incomplete_shell_input_uses_profile_authority() {
    for profile in PermissionProfile::all() {
        for input in incomplete_shell_inputs() {
            for (title, kind) in [
                ("Bash", None),
                ("Bash", Some(ToolKind::Execute)),
                ("Run the build", Some(ToolKind::Execute)),
                ("shell", None),
            ] {
                let mut policy = policy(*profile, false);
                let request = request(title, input.clone(), kind);
                let resolution = policy.evaluate_request(&request);
                let (outcome, decision) = match profile {
                    PermissionProfile::Yolo => (PermissionOutcome::Allow, PolicyDecision::Allow),
                    _ => (PermissionOutcome::Ask, PolicyDecision::Ask(RiskTier::High)),
                };
                assert_eq!(resolution.risk, RiskTier::High, "{profile} {title} {input}");
                assert_eq!(resolution.source, PermissionSource::Profile, "{input}");
                assert_eq!(resolution.outcome, outcome, "{profile} {title} {input}");
                assert_eq!(
                    policy.decide(&policy_request(&request)),
                    decision,
                    "{input}"
                );
            }
        }
    }
}

#[test]
fn a_gate_command_is_batch_approved_by_evaluate() {
    let mut policy = CockpitPolicy::new(
        PermissionProfile::Cautious,
        Vec::new(),
        vec!["cargo nextest run --workspace".to_owned()],
        Vec::new(),
        false,
    )
    .audit_to(None);
    let decision = policy.decide(&policy_request(&request(
        "Bash",
        serde_json::json!({"command": "custom-runner && cargo nextest run --workspace"}),
        None,
    )));
    assert_eq!(decision, PolicyDecision::Allow);
}

#[test]
fn evaluate_decisions_land_in_the_audit_log() {
    let dir = TempDir::new().expect("a temp dir");
    let log = dir.path().join("permissions.log");
    let mut policy = CockpitPolicy::new(
        PermissionProfile::Standard,
        Vec::new(),
        Vec::new(),
        Vec::new(),
        false,
    )
    .audit_to(Some(log.clone()));

    // Allowed by the profile → audited as allowed.
    policy.decide(&policy_request(&request(
        "Read",
        serde_json::json!({"file_path": "/src/main.rs"}),
        None,
    )));
    // Asked → audited as not allowed.
    policy.decide(&policy_request(&request(
        "Write",
        serde_json::json!({"file_path": "/src/new.rs"}),
        None,
    )));

    let entries = audit_entries(&log);
    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0].tool_name, "Read");
    assert!(entries[0].allowed);
    assert_eq!(entries[1].tool_name, "Write");
    assert!(!entries[1].allowed);
}

#[test]
fn audit_entries_carry_the_actor_and_respect_retention() {
    let dir = TempDir::new().expect("a temp dir");
    let log = dir.path().join("permissions.log");
    let mut policy = CockpitPolicy::new(
        PermissionProfile::Standard,
        Vec::new(),
        Vec::new(),
        Vec::new(),
        false,
    )
    .audit_to(Some(log.clone()))
    .acting_as(Some("dev@example.com".to_string()))
    .retain(AuditRetention {
        days: None,
        max_entries: Some(2),
    });

    for path in ["/src/a.rs", "/src/b.rs", "/src/c.rs"] {
        policy.decide(&policy_request(&request(
            "Read",
            serde_json::json!({"file_path": path}),
            None,
        )));
    }

    let entries = audit_entries(&log);
    assert_eq!(entries.len(), 2, "retention caps the log on append");
    assert!(
        entries
            .iter()
            .all(|e| e.actor.as_deref() == Some("dev@example.com"))
    );
    // Chronological order survives the rewrite.
    assert!(entries[0].timestamp <= entries[1].timestamp);
}

#[test]
fn denies_and_auto_approvals_are_not_audited() {
    let dir = TempDir::new().expect("a temp dir");
    let log = dir.path().join("permissions.log");
    let mut policy = CockpitPolicy::new(
        PermissionProfile::Standard,
        Vec::new(),
        Vec::new(),
        vec![PathBuf::from("/project/ROADMAP.md")],
        true,
    )
    .audit_to(Some(log.clone()));

    policy.decide(&policy_request(&request(
        "Write",
        serde_json::json!({"file_path": "/project/ROADMAP.md"}),
        None,
    )));
    policy.decide(&policy_request(&request(
        "anything",
        serde_json::json!({}),
        Some(ToolKind::Execute),
    )));

    assert!(
        audit_entries(&log).is_empty(),
        "as the legacy handler: only evaluate decisions are logged"
    );
}

#[test]
fn magistrs_public_mcp_tools_reach_their_native_authorization_outside_workflow() {
    let names = crate::unified_mcp::DIRECT
        .iter()
        .map(|(_, public, _)| *public)
        .chain(["discover", "execute"]);
    for name in names {
        let mut policy = policy(PermissionProfile::Cautious, false);
        let decision = policy.decide(&policy_request(&request(
            &format!("mcp__magistr__{name}"),
            serde_json::json!({}),
            Some(ToolKind::Other),
        )));
        assert_eq!(decision, PolicyDecision::Allow, "{name}");
    }
}

#[test]
fn workflow_mode_auto_approves_every_project_mcp_tool() {
    // The session's MCP servers are exactly the project's own under the
    // agent's --strict-mcp-config; an unanswered ask parks the loop.
    let mut policy = policy(PermissionProfile::Standard, true);
    for tool in ["mcp__magistr__read", "mcp__project_docs__search"] {
        assert_eq!(
            policy.decide(&policy_request(&request(
                tool,
                serde_json::json!({}),
                Some(ToolKind::Other)
            ))),
            PolicyDecision::Allow,
            "{tool}"
        );
    }
}

#[test]
fn outside_workflow_other_servers_and_unknown_operations_use_the_profile() {
    for tool in [
        "mcp__project_docs__read",
        "mcp__magistr_tools__read",
        "mcp__magistr__unknown_operation",
        "magistr_gate",
        "magistr-read",
    ] {
        let mut policy = policy(PermissionProfile::Standard, false);
        let decision = policy.decide(&policy_request(&request(
            tool,
            serde_json::json!({}),
            Some(ToolKind::Other),
        )));
        assert!(matches!(decision, PolicyDecision::Ask(_)), "{tool}");
    }
}

#[test]
fn unified_mcp_allowance_preserves_governance_denials_and_confirmations() {
    let tool = "mcp__magistr__execute";
    for (effect, expected) in [
        (Effect::Deny, PolicyDecision::Deny),
        (
            Effect::RequireApproval,
            PolicyDecision::Ask(RiskTier::Medium),
        ),
    ] {
        let mut policy = policy(PermissionProfile::Cautious, false).governed_by(
            GovernancePolicy {
                rules: vec![PolicyRule::new(
                    "review-composed-work",
                    PolicyCondition::tool(tool),
                    effect,
                )],
                ..GovernancePolicy::default()
            }
            .compile(),
        );
        let decision = policy.decide(&policy_request(&request(
            tool,
            serde_json::json!({"source": "return magistr.call('read', {file:'names.py'});"}),
            Some(ToolKind::Other),
        )));
        assert_eq!(decision, expected);
    }
}

// ── Governance policy ─────────────────────────────────────────

use magistr_engine::policy::{
    GovernancePolicy, PolicyCondition, PolicyEffect as Effect, PolicyEngine, PolicyRule,
};

fn fetch_request() -> AcpPermissionRequest {
    request(
        "WebFetch",
        serde_json::json!({"url": "https://x.dev"}),
        None,
    )
}

fn write_request(path: &str) -> AcpPermissionRequest {
    request(
        "Write",
        serde_json::json!({ "file_path": path }),
        Some(ToolKind::Edit),
    )
}

fn governed(profile: PermissionProfile, governance: GovernancePolicy) -> CockpitPolicy {
    CockpitPolicy::new(profile, Vec::new(), Vec::new(), Vec::new(), true)
        .governed_by(governance.compile())
        .audit_to(None)
}

#[test]
fn the_org_policy_denies_what_even_yolo_workflow_mode_would_allow() {
    // Workflow mode + yolo is the most permissive configuration there is.
    let mut ungoverned = policy(PermissionProfile::Yolo, true);
    assert_eq!(
        ungoverned.decide(&policy_request(&fetch_request())),
        PolicyDecision::Allow
    );

    let mut under = governed(
        PermissionProfile::Yolo,
        GovernancePolicy {
            tool_denylist: vec!["WebFetch".into()],
            ..GovernancePolicy::default()
        },
    );
    assert_eq!(
        under.decide(&policy_request(&fetch_request())),
        PolicyDecision::Deny
    );
}

#[test]
fn a_protected_path_from_the_roster_is_denied() {
    let mut under = governed(
        PermissionProfile::Yolo,
        GovernancePolicy {
            protected_paths: vec!["*.env".into()],
            ..GovernancePolicy::default()
        },
    );
    assert_eq!(
        under.decide(&policy_request(&write_request("/repo/.env"))),
        PolicyDecision::Deny
    );
    assert_eq!(
        under.decide(&policy_request(&write_request("/repo/src/main.rs"))),
        PolicyDecision::Allow
    );
}

#[test]
fn a_require_approval_rule_turns_an_auto_approval_into_a_prompt() {
    let mut under = governed(
        PermissionProfile::Yolo,
        GovernancePolicy {
            rules: vec![PolicyRule::new(
                "ask-before-writes",
                PolicyCondition::tool("Write"),
                Effect::RequireApproval,
            )],
            ..GovernancePolicy::default()
        },
    );
    assert_eq!(
        under.decide(&policy_request(&write_request("/repo/src/main.rs"))),
        PolicyDecision::Ask(RiskTier::Medium)
    );
}

#[test]
fn a_warn_rule_permits_the_call_and_is_only_recorded() {
    let mut under = governed(
        PermissionProfile::Yolo,
        GovernancePolicy {
            rules: vec![PolicyRule::new(
                "note-the-fetches",
                PolicyCondition::tool("WebFetch"),
                Effect::Warn,
            )],
            ..GovernancePolicy::default()
        },
    );
    assert_eq!(
        under.decide(&policy_request(&fetch_request())),
        PolicyDecision::Allow
    );
}

#[test]
fn the_policy_cannot_loosen_a_local_denial() {
    // The local protected-path list denies this write; a blanket Allow
    // rule in the roster must not un-deny it.
    let allow_everything = GovernancePolicy {
        rules: vec![PolicyRule::new(
            "allow-everything",
            PolicyCondition::Always,
            Effect::Allow,
        )],
        ..GovernancePolicy::default()
    };
    let mut under = CockpitPolicy::new(
        PermissionProfile::Yolo,
        Vec::new(),
        Vec::new(),
        vec![PathBuf::from("/repo/ROADMAP.md")],
        true,
    )
    .governed_by(allow_everything.compile())
    .audit_to(None);
    assert_eq!(
        under.decide(&policy_request(&write_request("/repo/ROADMAP.md"))),
        PolicyDecision::Deny
    );
}

#[test]
fn a_governed_denial_is_audited_by_rule_id() {
    let dir = TempDir::new().expect("tempdir");
    let log = dir.path().join("audit.jsonl");
    let governance = GovernancePolicy {
        tool_denylist: vec!["WebFetch".into()],
        ..GovernancePolicy::default()
    };
    let mut under = CockpitPolicy::new(
        PermissionProfile::Yolo,
        Vec::new(),
        Vec::new(),
        Vec::new(),
        true,
    )
    .governed_by(governance.compile())
    .audit_to(Some(log.clone()));
    assert_eq!(
        under.decide(&policy_request(&fetch_request())),
        PolicyDecision::Deny
    );

    let entries = audit_entries(&log);
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].reason, "policy rule: tool-denylist:WebFetch");
    assert!(!entries[0].allowed);
}

#[test]
fn an_empty_policy_leaves_every_decision_exactly_as_it_was() {
    let empty = PolicyEngine::default();
    let target = write_request("/repo/src/main.rs");
    for profile in PermissionProfile::all() {
        let mut plain = policy(*profile, false);
        let mut under = CockpitPolicy::new(*profile, Vec::new(), Vec::new(), Vec::new(), false)
            .governed_by(empty.clone())
            .audit_to(None);
        assert_eq!(
            plain.decide(&policy_request(&target)),
            under.decide(&policy_request(&target))
        );
    }
}

// ── The charter ───────────────────────────────

mod charter {
    use super::*;
    use crate::agent::permission_request::permission_request as policy_request;
    use crate::permissions::CharterStore;
    #[cfg(unix)]
    use magistr_application::sessions::PermissionKind;
    use magistr_engine::policy::{
        Charter, GovernancePolicy, PolicyCondition, PolicyEffect as Effect, PolicyRule,
    };
    use std::path::Path;

    fn write_under(path: &str) -> AcpPermissionRequest {
        request(
            "Write",
            serde_json::json!({ "file_path": path }),
            Some(ToolKind::Edit),
        )
    }

    /// Cautious asks about every write; rooted at `/repo`, so the
    /// scope of a write under it is `Write · crates/<name>`.
    fn cautious(charter: Charter, store: Option<CharterStore>) -> CockpitPolicy {
        CockpitPolicy::new(
            PermissionProfile::Cautious,
            Vec::new(),
            Vec::new(),
            Vec::new(),
            false,
        )
        .rooted_at(Some(Path::new("/repo").to_path_buf()))
        .audit_to(None)
        .chartered(charter, store)
    }

    #[cfg(unix)]
    fn asks(decision: PolicyDecision) -> bool {
        matches!(decision, PolicyDecision::Ask(_))
    }

    #[cfg(unix)]
    #[test]
    fn a_delegated_scope_is_allowed_where_the_profile_would_ask() {
        let mut plain = cautious(Charter::default(), None);
        assert!(asks(plain.decide(&policy_request(&write_under(
            "/repo/crates/magistr-desktop/src/view.rs"
        )))));

        let mut charter = Charter::default();
        charter.delegate(PolicyRule::new(
            "delegate:write:crates/magistr-desktop",
            PolicyCondition::all_of(vec![
                PolicyCondition::tool("Write"),
                PolicyCondition::path("*crates/magistr-desktop/*"),
            ]),
            Effect::Allow,
        ));
        let mut under = cautious(charter, None);
        assert_eq!(
            under.decide(&policy_request(&write_under(
                "/repo/crates/magistr-desktop/src/view.rs"
            ))),
            PolicyDecision::Allow
        );
        assert!(
            asks(under.decide(&policy_request(&write_under(
                "/repo/crates/magistr-cockpit/src/view.rs"
            )))),
            "outside the scope the profile still asks"
        );
    }

    /// The lattice: the roster's Deny folds over a delegated Allow.
    #[test]
    fn the_roster_caps_a_delegated_allow() {
        let mut charter = Charter::default();
        charter.delegate(PolicyRule::new(
            "delegate:write:*",
            PolicyCondition::tool("Write"),
            Effect::Allow,
        ));
        let mut under = cautious(charter, None).governed_by(
            GovernancePolicy {
                protected_paths: vec!["*.env".into()],
                ..GovernancePolicy::default()
            }
            .compile(),
        );
        assert_eq!(
            under.decide(&policy_request(&write_under("/repo/.env"))),
            PolicyDecision::Deny
        );
        assert_eq!(
            under.decide(&policy_request(&write_under("/repo/src/main.rs"))),
            PolicyDecision::Allow,
            "where the roster says nothing the widening holds"
        );
    }

    #[cfg(unix)]
    #[test]
    fn an_always_answer_writes_the_rule_the_next_session_enforces() {
        let dir = TempDir::new().expect("a temp dir");
        let store = CharterStore::at(dir.path());
        let mut first = cautious(Charter::default(), Some(store.clone()));
        let ask = write_under("/repo/crates/x/src/a.rs");
        assert!(asks(first.decide(&policy_request(&ask))));
        first.answered(&policy_request(&ask), PermissionKind::AllowAlways);
        assert_eq!(
            first.decide(&policy_request(&write_under("/repo/crates/x/src/b.rs"))),
            PolicyDecision::Allow,
            "the rule holds for the rest of this session"
        );
        let tally = first.charter().tally("write:crates/x").expect("tallied");
        assert_eq!((tally.allowed, tally.denied), (1, 0));
        assert_eq!(tally.subject, "Write \u{b7} crates/x");

        // Next session: a fresh policy over what the store kept.
        let saved = store.load().expect("saved");
        assert_eq!(
            saved
                .delegation("delegate:write:crates/x")
                .map(|r| r.effect),
            Some(Effect::Allow)
        );
        let mut next = cautious(saved, Some(store));
        assert_eq!(
            next.decide(&policy_request(&write_under("/repo/crates/x/src/c.rs"))),
            PolicyDecision::Allow
        );
        assert!(asks(next.decide(&policy_request(&write_under(
            "/repo/crates/y/src/c.rs"
        )))));
    }

    #[cfg(unix)]
    #[test]
    fn a_never_answer_denies_and_a_written_condition_asks_where_yolo_would_allow() {
        let yolo = || {
            CockpitPolicy::new(
                PermissionProfile::Yolo,
                Vec::new(),
                Vec::new(),
                Vec::new(),
                false,
            )
            .rooted_at(Some(Path::new("/repo").to_path_buf()))
            .audit_to(None)
        };
        let ask = write_under("/repo/crates/x/src/a.rs");

        let mut never = yolo();
        assert_eq!(never.decide(&policy_request(&ask)), PolicyDecision::Allow);
        never.answered(&policy_request(&ask), PermissionKind::RejectAlways);
        assert_eq!(never.decide(&policy_request(&ask)), PolicyDecision::Deny);
        assert_eq!(
            never.charter().tally("write:crates/x").map(|t| t.denied),
            Some(1)
        );

        let mut conditioned = yolo();
        conditioned.constrain(&policy_request(&ask), "  allow, stop if other rows change ");
        assert!(asks(conditioned.decide(&policy_request(&ask))));
        assert_eq!(
            conditioned
                .charter()
                .delegation("ask:write:crates/x")
                .map(|r| r.description.as_str()),
            Some("allow, stop if other rows change"),
            "the rule carries the reader's words"
        );
        let mut blank = yolo();
        blank.constrain(&policy_request(&ask), "   ");
        assert!(blank.charter().is_empty(), "no words, no rule");
    }

    #[cfg(unix)]
    #[test]
    fn once_answers_only_tally_and_an_accepted_offer_writes_the_rule() {
        let mut policy = cautious(Charter::default(), None);
        let ask = write_under("/repo/crates/x/src/a.rs");
        for _ in 0..Charter::OFFER_AFTER {
            policy.answered(&policy_request(&ask), PermissionKind::AllowOnce);
        }
        assert!(
            asks(policy.decide(&policy_request(&ask))),
            "once is not a rule"
        );
        let offers = policy.charter().offers();
        assert_eq!(offers.len(), 1);
        assert_eq!(offers[0].key, "write:crates/x");
        assert!(!policy.delegate("nowhere"));
        assert!(policy.delegate("write:crates/x"));
        assert_eq!(policy.decide(&policy_request(&ask)), PolicyDecision::Allow);
        assert!(policy.charter().offers().is_empty());
    }

    #[test]
    fn a_delegation_is_audited_by_its_id() {
        let dir = TempDir::new().expect("a temp dir");
        let log = dir.path().join("permissions.log");
        let mut charter = Charter::default();
        charter.delegate(PolicyRule::new(
            "delegate:write:crates/x",
            PolicyCondition::tool("Write"),
            Effect::Allow,
        ));
        let mut policy = cautious(charter, None).audit_to(Some(log.clone()));
        assert_eq!(
            policy.decide(&policy_request(&write_under("/repo/crates/x/src/a.rs"))),
            PolicyDecision::Allow
        );
        let entries = audit_entries(&log);
        assert_eq!(entries.len(), 1);
        assert!(entries[0].allowed);
        assert_eq!(entries[0].reason, "delegated: delegate:write:crates/x");
    }
}

#[cfg(unix)]
#[test]
fn remembered_refusal_caps_workflow_shortcuts() {
    let mut policy = policy(PermissionProfile::Yolo, true);
    let target = write_request("/repo/src/main.rs");
    policy.answered(&policy_request(&target), PermissionKind::RejectAlways);
    assert_eq!(
        policy.decide(&policy_request(&target)),
        PolicyDecision::Deny
    );
}

#[test]
fn written_condition_caps_safe_command_shortcuts() {
    let mut policy = policy(PermissionProfile::Yolo, false);
    let target = request(
        "Bash",
        serde_json::json!({"command":"cargo test"}),
        Some(ToolKind::Execute),
    );
    policy.constrain(
        &policy_request(&target),
        "ask before running expensive tests",
    );
    assert!(matches!(
        policy.decide(&policy_request(&target)),
        PolicyDecision::Ask(_)
    ));
}

#[test]
fn protected_targets_include_delete_and_move_requests() {
    for kind in [ToolKind::Delete, ToolKind::Move] {
        let mut policy = CockpitPolicy::new(
            PermissionProfile::Yolo,
            vec![],
            vec![],
            vec![PathBuf::from("/repo/ROADMAP.md")],
            true,
        )
        .audit_to(None);
        let target = request(
            "change target",
            serde_json::json!({"paths":["/repo/ordinary.rs", "/repo/ROADMAP.md"]}),
            Some(kind),
        );
        assert_eq!(
            policy.decide(&policy_request(&target)),
            PolicyDecision::Deny,
            "{kind:?}"
        );
    }
}

#[test]
fn a_remembered_path_allow_cannot_widen_a_multi_target_call() {
    let mut policy =
        policy(PermissionProfile::Cautious, false).rooted_at(Some(PathBuf::from("/repo")));
    policy.answered(
        &policy_request(&write_request("/repo/crates/allowed/a.rs")),
        PermissionKind::AllowAlways,
    );
    let target = request(
        "Write",
        serde_json::json!({"paths":["/repo/crates/allowed/b.rs", "/repo/crates/other/c.rs"]}),
        Some(ToolKind::Edit),
    );
    assert!(matches!(
        policy.decide(&policy_request(&target)),
        PolicyDecision::Ask(_)
    ));
}

#[cfg(unix)]
#[test]
fn shared_decisions_carry_stable_request_scope_and_behavior_version() {
    let make = || {
        policy(PermissionProfile::Cautious, false)
            .rooted_at(Some(PathBuf::from("/repo")))
            .on_model(Some("model-a".into()))
    };
    let target = write_request("src/main.rs");
    let mut first = make();
    let baseline = first.evaluate_request(&target);
    assert_eq!(baseline, make().evaluate_request(&target));
    assert!(!baseline.reason.is_empty());
    assert!(!baseline.scope.is_empty());
    let different_target = first.evaluate_request(&write_request("src/other.rs"));
    assert_ne!(baseline.scope, different_target.scope);
    assert_eq!(baseline.policy_version, different_target.policy_version);
    let other_project = make()
        .rooted_at(Some(PathBuf::from("/other")))
        .evaluate_request(&target);
    assert_ne!(baseline.scope, other_project.scope);
    assert_ne!(baseline.policy_version, other_project.policy_version);
    let other_model = make()
        .on_model(Some("model-b".into()))
        .evaluate_request(&target);
    assert_ne!(baseline.scope, other_model.scope);
    assert_ne!(baseline.policy_version, other_model.policy_version);
    first.answered(&policy_request(&target), PermissionKind::AllowOnce);
    assert_eq!(
        baseline.policy_version,
        first.policy_version(),
        "tallies do not change authorization"
    );
    first.answered(&policy_request(&target), PermissionKind::RejectAlways);
    let refused = first.evaluate_request(&target);
    assert_eq!(refused.outcome, PermissionOutcome::Deny);
    assert_ne!(baseline.policy_version, refused.policy_version);
    assert_eq!(baseline.scope, refused.scope);
}

#[cfg(unix)]
#[test]
fn governance_matches_normalized_absolute_and_project_relative_paths() {
    for pattern in ["src/private/*", "/repo/src/private/*"] {
        let mut policy = policy(PermissionProfile::Yolo, true)
            .rooted_at(Some(PathBuf::from("/repo")))
            .governed_by(PolicyEngine::new(vec![PolicyRule::new(
                "private",
                PolicyCondition::path(pattern),
                Effect::Deny,
            )]));
        for target in [
            "./src/private/key",
            "src/ordinary/../private/key",
            "/repo/src/private/key",
        ] {
            assert_eq!(
                policy.decide(&policy_request(&write_request(target))),
                PolicyDecision::Deny,
                "{pattern}: {target}"
            );
        }
    }
}

#[test]
fn remembered_allow_does_not_follow_a_path_that_escapes_its_scope() {
    let mut policy =
        policy(PermissionProfile::Cautious, false).rooted_at(Some(PathBuf::from("/repo")));
    policy.answered(
        &policy_request(&write_request("crates/allowed/a.rs")),
        PermissionKind::AllowAlways,
    );
    assert!(matches!(
        policy.decide(&policy_request(&write_request(
            "crates/allowed/../private/key"
        ))),
        PolicyDecision::Ask(_)
    ));
    policy.answered(
        &policy_request(&write_request("/outside/a.rs")),
        PermissionKind::AllowAlways,
    );
    assert!(matches!(
        policy.decide(&policy_request(&write_request("crates/other/a.rs"))),
        PolicyDecision::Ask(_)
    ));
}

#[test]
fn standing_answers_do_not_turn_glob_shaped_tool_titles_into_wildcard_rules() {
    let mut policy = policy(PermissionProfile::Cautious, false);
    let target = request("*", serde_json::json!({}), None);
    policy.answered(&policy_request(&target), PermissionKind::AllowAlways);
    assert!(policy.charter().delegations.is_empty());
    assert!(matches!(
        policy.decide(&policy_request(&write_request("src/main.rs"))),
        PolicyDecision::Ask(_)
    ));
}
#[test]
fn model_choices_use_the_same_frozen_policy_as_permission_decisions() {
    let governance = magistr_engine::policy::GovernancePolicy {
        approved_models: vec!["approved".into()],
        ..Default::default()
    };
    let mut policy = policy(PermissionProfile::Standard, false)
        .governed_by(governance.compile())
        .on_model(Some("initial".into()));
    assert!(policy.can_use_model("approved"));
    assert!(!policy.can_use_model("other"));
    policy.model_changed(Some("approved".into()));
    assert_eq!(policy.model.as_deref(), Some("approved"));
}
