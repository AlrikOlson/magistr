use alloc::{string::ToString, vec};

use super::*;

use crate::policy::PolicyRequest;

fn tool_policy() -> GovernancePolicy {
    GovernancePolicy {
        tool_allowlist: vec!["Read".into(), "Write".into(), "mcp__magistr__*".into()],
        tool_denylist: vec!["WebFetch".into()],
        ..GovernancePolicy::default()
    }
}

#[test]
fn an_empty_policy_is_unset_and_compiles_to_nothing() {
    let policy = GovernancePolicy::default();
    assert!(!policy.is_set());
    assert!(policy.compile().is_empty());
}

#[test]
fn any_shorthand_makes_the_policy_set() {
    for policy in [
        GovernancePolicy {
            tool_denylist: vec!["Bash".into()],
            ..GovernancePolicy::default()
        },
        GovernancePolicy {
            protected_paths: vec!["*.lock".into()],
            ..GovernancePolicy::default()
        },
        GovernancePolicy {
            required_gates: vec!["test".into()],
            ..GovernancePolicy::default()
        },
        GovernancePolicy {
            max_model_tier: Some(ModelTier::Standard),
            ..GovernancePolicy::default()
        },
    ] {
        assert!(policy.is_set());
    }
}

// ── Tool approval ──────────────────────────────────────────────────

#[test]
fn an_allowlisted_tool_passes_and_an_unlisted_one_is_denied() {
    let engine = tool_policy().compile();
    assert_eq!(
        engine.evaluate(&PolicyRequest::tool("Read")).effect,
        PolicyEffect::Allow
    );
    assert_eq!(
        engine
            .evaluate(&PolicyRequest::tool("mcp__magistr__gate"))
            .effect,
        PolicyEffect::Allow
    );
    let denied = engine.evaluate(&PolicyRequest::tool("Bash"));
    assert_eq!(denied.effect, PolicyEffect::Deny);
    assert_eq!(denied.binding.as_deref(), Some("tool-allowlist"));
}

#[test]
fn the_denylist_beats_the_allowlist() {
    let policy = GovernancePolicy {
        tool_allowlist: vec!["Web*".into()],
        tool_denylist: vec!["WebFetch".into()],
        ..GovernancePolicy::default()
    };
    let engine = policy.compile();
    assert_eq!(
        engine.evaluate(&PolicyRequest::tool("WebSearch")).effect,
        PolicyEffect::Allow
    );
    let denied = engine.evaluate(&PolicyRequest::tool("WebFetch"));
    assert_eq!(denied.effect, PolicyEffect::Deny);
    assert_eq!(denied.binding.as_deref(), Some("tool-denylist:WebFetch"));
}

#[test]
fn the_allowlist_does_not_fire_on_a_request_with_no_tool() {
    // A model-only or gate-only request must not be denied by a *tool*
    // allowlist it says nothing about.
    let engine = tool_policy().compile();
    let request = PolicyRequest::new().with_model("claude-opus-5");
    assert_eq!(engine.evaluate(&request).effect, PolicyEffect::Allow);
}

// ── File protection ────────────────────────────────────────────────

#[test]
fn a_protected_path_is_denied_but_overridable() {
    let policy = GovernancePolicy {
        protected_paths: vec!["*team.json".into(), "*/migrations/*".into()],
        ..GovernancePolicy::default()
    };
    let engine = policy.compile();
    let decision = engine.evaluate(&PolicyRequest::tool("Write").with_path(".magistr/team.json"));
    assert_eq!(decision.effect, PolicyEffect::Deny);
    assert!(decision.override_allowed);
    assert_eq!(
        decision.binding.as_deref(),
        Some("protected-path:*team.json")
    );

    assert_eq!(
        engine
            .evaluate(&PolicyRequest::tool("Write").with_path("src/main.rs"))
            .effect,
        PolicyEffect::Allow
    );
}

// ── Quality gates ──────────────────────────────────────────────────

#[test]
fn missing_required_gates_names_what_did_not_pass() {
    let policy = GovernancePolicy {
        required_gates: vec!["check".into(), "test".into(), "lint".into()],
        ..GovernancePolicy::default()
    };
    assert!(
        policy
            .missing_required_gates(&["check", "test", "lint"])
            .is_empty()
    );
    assert_eq!(
        policy.missing_required_gates(&["Check", "lint"]),
        vec!["test".to_string()]
    );
    assert_eq!(
        policy.missing_required_gates::<&str>(&[]),
        vec!["check".to_string(), "test".to_string(), "lint".to_string()]
    );
}

#[test]
fn a_policy_with_no_required_gates_never_reports_one_missing() {
    let policy = GovernancePolicy::default();
    assert!(policy.missing_required_gates::<&str>(&[]).is_empty());
    assert!(!policy.allow_gate_bypass);
}

// ── Model governance ───────────────────────────────────────────────

fn model_policy() -> GovernancePolicy {
    GovernancePolicy {
        approved_models: vec!["claude-*".into()],
        model_tiers: vec![
            ModelTierRule::new("claude-haiku-*", ModelTier::Economy),
            ModelTierRule::new("claude-sonnet-*", ModelTier::Standard),
            ModelTierRule::new("claude-opus-*", ModelTier::Frontier),
        ],
        max_model_tier: Some(ModelTier::Standard),
        fallback_model: Some("claude-sonnet-5".into()),
        ..GovernancePolicy::default()
    }
}

#[test]
fn tier_of_classifies_by_family() {
    let policy = model_policy();
    assert_eq!(policy.tier_of("claude-opus-5"), Some(ModelTier::Frontier));
    assert_eq!(policy.tier_of("claude-haiku-4-5"), Some(ModelTier::Economy));
    assert_eq!(policy.tier_of("some-other-model"), None);
}

#[test]
fn a_model_above_the_tier_cap_is_denied() {
    let engine = model_policy().compile();
    let decision = engine.evaluate(&PolicyRequest::new().with_model("claude-opus-5"));
    assert_eq!(decision.effect, PolicyEffect::Deny);
    assert_eq!(
        decision.binding.as_deref(),
        Some("model-tier:claude-opus-*")
    );
}

#[test]
fn an_unapproved_model_is_denied_by_the_allowlist() {
    let engine = model_policy().compile();
    let decision = engine.evaluate(&PolicyRequest::new().with_model("gpt-9"));
    assert_eq!(decision.effect, PolicyEffect::Deny);
    assert_eq!(decision.binding.as_deref(), Some("model-allowlist"));
}

#[test]
fn routing_returns_the_request_when_it_is_approved() {
    assert_eq!(
        model_policy().route_model("claude-sonnet-5"),
        Some("claude-sonnet-5")
    );
}

#[test]
fn routing_falls_back_when_the_request_is_refused() {
    assert_eq!(
        model_policy().route_model("claude-opus-5"),
        Some("claude-sonnet-5")
    );
}

#[test]
fn routing_refuses_rather_than_route_to_a_fallback_that_is_itself_denied() {
    let policy = GovernancePolicy {
        approved_models: vec!["claude-sonnet-*".into()],
        fallback_model: Some("gpt-9".into()),
        ..GovernancePolicy::default()
    };
    assert_eq!(policy.route_model("claude-opus-5"), None);
}

#[test]
fn routing_with_no_model_policy_passes_anything_through() {
    assert_eq!(
        GovernancePolicy::default().route_model("anything-at-all"),
        Some("anything-at-all")
    );
}

// ── Serde ──────────────────────────────────────────────────────────

#[test]
fn an_absent_policy_deserializes_to_the_empty_one() {
    let policy: GovernancePolicy = serde_json::from_str("{}").expect("deserialize");
    assert_eq!(policy, GovernancePolicy::default());
}

#[test]
fn an_empty_policy_serializes_to_an_empty_object() {
    let json = serde_json::to_string(&GovernancePolicy::default()).expect("serialize");
    assert_eq!(json, r#"{"allow_gate_bypass":false}"#);
}

#[test]
fn round_trips_through_the_roster_json() {
    let policy = model_policy();
    let json = serde_json::to_string(&policy).expect("serialize");
    let back: GovernancePolicy = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(back, policy);
}
