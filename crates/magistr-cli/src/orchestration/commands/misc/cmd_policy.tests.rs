use super::*;

use crate::config::inputs::ProjectInputs;
use crate::output::JsonRenderer;
use magistr_engine::policy::ModelTier;

/// A capture with nothing set — no identity override, no data directory.
fn inputs(dir: &Path) -> ProjectInputs {
    ProjectInputs::from_lookup(dir, |_| None)
}

fn governed_project() -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("tempdir");
    let identity = crate::team::Identity::resolve(dir.path(), &inputs(dir.path()));
    let mut config = TeamConfig::new_with_owner(&identity);
    config.policy = GovernancePolicy {
        tool_denylist: vec!["WebFetch".into()],
        protected_paths: vec!["*.env".into()],
        required_gates: vec!["test".into()],
        approved_models: vec!["claude-*".into()],
        max_model_tier: Some(ModelTier::Standard),
        ..GovernancePolicy::default()
    };
    config.save(dir.path()).expect("save roster");
    dir
}

#[test]
fn show_on_an_ungoverned_project_succeeds_and_says_so() {
    let dir = tempfile::tempdir().expect("tempdir");
    let renderer = JsonRenderer::new();
    cmd_policy(
        dir.path(),
        &inputs(dir.path()),
        Some(PolicyAction::Show),
        &renderer,
    )
    .expect("show");
    assert!(!TeamConfig::governance_for(dir.path()).is_set());
}

#[test]
fn show_compiles_the_roster_policy() {
    let dir = governed_project();
    let policy = TeamConfig::governance_for(dir.path());
    assert!(policy.is_set());
    // denylist + protected path + model allowlist = three rules.
    assert_eq!(policy.compile().len(), 3);
    cmd_policy(
        dir.path(),
        &inputs(dir.path()),
        Some(PolicyAction::Show),
        &JsonRenderer::new(),
    )
    .expect("show");
}

#[test]
fn check_refuses_a_request_that_names_nothing() {
    let dir = governed_project();
    let result = cmd_policy(
        dir.path(),
        &inputs(dir.path()),
        Some(PolicyAction::Check {
            tool: None,
            command: None,
            path: Vec::new(),
            model: None,
        }),
        &JsonRenderer,
    );
    assert!(result.is_err());
}

#[test]
fn check_reports_the_effect_for_a_denied_tool() {
    let dir = governed_project();
    cmd_policy(
        dir.path(),
        &inputs(dir.path()),
        Some(PolicyAction::Check {
            tool: Some("WebFetch".into()),
            command: None,
            path: Vec::new(),
            model: None,
        }),
        &JsonRenderer,
    )
    .expect("check");

    // The decision the command prints is the engine's, unchanged.
    let policy = TeamConfig::governance_for(dir.path());
    let request = policy_request_for("WebFetch", &serde_json::json!({}), None);
    let decision = policy.compile().evaluate(&request);
    assert_eq!(decision.effect, PolicyEffect::Deny);
    assert_eq!(decision.binding.as_deref(), Some("tool-denylist:WebFetch"));
}

#[test]
fn check_sees_a_path_through_the_same_request_shape_enforcement_uses() {
    let dir = governed_project();
    let policy = TeamConfig::governance_for(dir.path());
    let request = policy_request_for(
        "Write",
        &serde_json::json!({ "paths": ["/repo/.env"] }),
        None,
    );
    let decision = policy.compile().evaluate(&request);
    assert_eq!(decision.effect, PolicyEffect::Deny);
    assert!(decision.override_allowed);

    cmd_policy(
        dir.path(),
        &inputs(dir.path()),
        Some(PolicyAction::Check {
            tool: Some("Write".into()),
            command: None,
            path: vec!["/repo/.env".into()],
            model: None,
        }),
        &JsonRenderer,
    )
    .expect("check");
}

#[test]
fn check_reports_an_unapproved_model() {
    let dir = governed_project();
    let policy = TeamConfig::governance_for(dir.path());
    let request = policy_request_for("", &serde_json::json!({}), Some("gpt-9"));
    assert_eq!(
        policy.compile().evaluate(&request).binding.as_deref(),
        Some("model-allowlist")
    );

    cmd_policy(
        dir.path(),
        &inputs(dir.path()),
        Some(PolicyAction::Check {
            tool: None,
            command: None,
            path: Vec::new(),
            model: Some("gpt-9".into()),
        }),
        &JsonRenderer,
    )
    .expect("check");
}

#[test]
fn show_is_the_default_action() {
    let dir = governed_project();
    cmd_policy(dir.path(), &inputs(dir.path()), None, &JsonRenderer::new())
        .expect("default action");
}

#[test]
fn a_model_only_check_is_not_answered_by_the_tool_allowlist() {
    let dir = tempfile::tempdir().expect("tempdir");
    let identity = crate::team::Identity::resolve(dir.path(), &inputs(dir.path()));
    let mut config = TeamConfig::new_with_owner(&identity);
    config.policy = GovernancePolicy {
        tool_allowlist: vec!["Read".into()],
        approved_models: vec!["claude-*".into()],
        ..GovernancePolicy::default()
    };
    config.save(dir.path()).expect("save roster");

    // An approved model with no tool named must come back clean, not
    // denied by the tool allowlist through an empty tool name.
    cmd_policy(
        dir.path(),
        &inputs(dir.path()),
        Some(PolicyAction::Check {
            tool: None,
            command: None,
            path: Vec::new(),
            model: Some("claude-sonnet-5".into()),
        }),
        &JsonRenderer::new(),
    )
    .expect("check");

    let policy = TeamConfig::governance_for(dir.path());
    let mut request = policy_request_for("", &serde_json::json!({}), Some("claude-sonnet-5"));
    assert_eq!(
        policy.compile().evaluate(&request).binding.as_deref(),
        Some("tool-allowlist"),
        "an empty tool name trips the allowlist — which is why check clears it"
    );
    request.tool = None;
    request.risk = None;
    assert_eq!(
        policy.compile().evaluate(&request).effect,
        PolicyEffect::Allow
    );
}
