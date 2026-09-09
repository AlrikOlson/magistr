use super::*;

use magistr_engine::policy::{GovernancePolicy, ModelTier, ModelTierRule};

fn project_with(policy: GovernancePolicy) -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("tempdir");
    let identity = crate::team::Identity::resolve(dir.path());
    let mut config = TeamConfig::new_with_owner(&identity);
    config.policy = policy;
    config.save(dir.path()).expect("save roster");
    dir
}

#[test]
fn no_request_means_the_agents_own_default_stands() {
    let dir = project_with(GovernancePolicy {
        approved_models: vec!["claude-*".into()],
        ..GovernancePolicy::default()
    });
    assert_eq!(governed_model(dir.path(), None).expect("resolve"), None);
}

#[test]
fn an_ungoverned_project_takes_whatever_was_asked_for() {
    let dir = tempfile::tempdir().expect("tempdir");
    assert_eq!(
        governed_model(dir.path(), Some("gpt-9")).expect("resolve"),
        Some("gpt-9".to_string())
    );
}

#[test]
fn an_approved_model_comes_back_unchanged() {
    let dir = project_with(GovernancePolicy {
        approved_models: vec!["claude-*".into()],
        ..GovernancePolicy::default()
    });
    assert_eq!(
        governed_model(dir.path(), Some("claude-sonnet-5")).expect("resolve"),
        Some("claude-sonnet-5".to_string())
    );
}

#[test]
fn a_model_over_the_tier_cap_is_routed_to_the_fallback() {
    let dir = project_with(GovernancePolicy {
        model_tiers: vec![
            ModelTierRule::new("claude-opus-*", ModelTier::Frontier),
            ModelTierRule::new("claude-sonnet-*", ModelTier::Standard),
        ],
        max_model_tier: Some(ModelTier::Standard),
        fallback_model: Some("claude-sonnet-5".into()),
        ..GovernancePolicy::default()
    });
    assert_eq!(
        governed_model(dir.path(), Some("claude-opus-5")).expect("resolve"),
        Some("claude-sonnet-5".to_string())
    );
}

#[test]
fn a_refused_model_with_no_fallback_is_an_error_not_a_silent_downgrade() {
    let dir = project_with(GovernancePolicy {
        approved_models: vec!["claude-*".into()],
        ..GovernancePolicy::default()
    });
    let error = governed_model(dir.path(), Some("gpt-9")).expect_err("refused");
    assert!(error.to_string().contains("not approved"), "{error}");
}
