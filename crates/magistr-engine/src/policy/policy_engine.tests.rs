use super::*;

use crate::policy::{PolicyCondition, RiskTier};

fn engine() -> PolicyEngine {
    PolicyEngine::new(vec![
        PolicyRule::new(
            "no-force-push",
            PolicyCondition::command("*push --force*"),
            PolicyEffect::Deny,
        ),
        PolicyRule::new(
            "high-risk-asks",
            PolicyCondition::risk_at_least(RiskTier::High),
            PolicyEffect::RequireApproval,
        ),
        PolicyRule::new(
            "roster-is-precious",
            PolicyCondition::path("*team.json"),
            PolicyEffect::RequireApproval,
        )
        .overridable(),
    ])
}

#[test]
fn an_empty_engine_constrains_nothing() {
    let engine = PolicyEngine::default();
    assert!(engine.is_empty());
    assert_eq!(engine.len(), 0);
    let decision = engine.evaluate(&PolicyRequest::tool("Bash").with_command("rm -rf /"));
    assert_eq!(decision.effect, PolicyEffect::Allow);
    assert!(decision.matched.is_empty());
}

#[test]
fn evaluation_takes_the_tightest_matching_rule() {
    let request = PolicyRequest::tool("Bash")
        .with_command("git push --force")
        .with_risk(RiskTier::High);
    let decision = engine().evaluate(&request);
    assert_eq!(decision.effect, PolicyEffect::Deny);
    assert_eq!(decision.binding.as_deref(), Some("no-force-push"));
    assert_eq!(decision.matched, vec!["no-force-push", "high-risk-asks"]);
}

#[test]
fn an_overridable_rule_reports_its_escape_hatch() {
    let request = PolicyRequest::tool("Write").with_path(".magistr/team.json");
    let decision = engine().evaluate(&request);
    assert_eq!(decision.effect, PolicyEffect::RequireApproval);
    assert!(decision.override_allowed);
}

#[test]
fn tighten_never_loosens_a_local_decision() {
    let engine = engine();
    // No rule matches a plain read: the local decision survives intact.
    let read = PolicyRequest::tool("Read").with_path("src/main.rs");
    for local in PolicyEffect::ALL {
        assert_eq!(engine.tighten(local, &read).effect, local);
    }
}

#[test]
fn tighten_raises_a_permissive_local_decision_to_the_policys() {
    let engine = engine();
    let request = PolicyRequest::tool("Bash").with_command("git push --force");
    // Even a `yolo` profile saying Allow ends up denied.
    let decision = engine.tighten(PolicyEffect::Allow, &request);
    assert_eq!(decision.effect, PolicyEffect::Deny);
    assert_eq!(decision.binding.as_deref(), Some("no-force-push"));
}

#[test]
fn tighten_keeps_a_stricter_local_decision() {
    let engine = engine();
    let request = PolicyRequest::tool("Write").with_path(".magistr/team.json");
    // Policy says RequireApproval; the local profile already says Deny.
    let decision = engine.tighten(PolicyEffect::Deny, &request);
    assert_eq!(decision.effect, PolicyEffect::Deny);
}

#[test]
fn a_local_only_tightening_is_not_overridable() {
    // Nothing in the policy fired, so there is no rule to override.
    let decision = PolicyEngine::default().tighten(PolicyEffect::Deny, &PolicyRequest::new());
    assert_eq!(decision.effect, PolicyEffect::Deny);
    assert!(!decision.override_allowed);
    assert!(decision.binding.is_none());
}

#[test]
fn merging_can_only_add_strictness() {
    let stricter = PolicyEngine::new(vec![PolicyRule::new(
        "no-bash-at-all",
        PolicyCondition::tool("Bash"),
        PolicyEffect::Deny,
    )]);
    let merged = PolicyEngine::default().merged_with(&stricter);
    assert_eq!(merged.len(), 1);
    let request = PolicyRequest::tool("Bash").with_command("ls");
    assert_eq!(merged.evaluate(&request).effect, PolicyEffect::Deny);
}
