use super::*;
use crate::policy::{PolicyCondition, PolicyRule};
use alloc::vec;

fn context() -> PermissionContext {
    PermissionContext {
        mode: PermissionMode::Standard,
        profile_effect: PolicyEffect::RequireApproval,
        profile_reason: "profile default".into(),
        local_allowance: false,
        workflow_allowance: true,
        protected: false,
        scope: "project:/repo;tool:Write;path:src/main.rs".into(),
        policy_version: 42,
    }
}

#[test]
fn every_outcome_carries_scope_version_and_reason() {
    for (effect, outcome) in [
        (PolicyEffect::Allow, PermissionOutcome::Allow),
        (PolicyEffect::RequireApproval, PermissionOutcome::Ask),
        (PolicyEffect::Deny, PermissionOutcome::Deny),
    ] {
        let context = PermissionContext {
            profile_effect: effect,
            ..context()
        };
        let decision = PermissionPolicyService::evaluate(
            &PolicyRequest::tool("Write"),
            &context,
            &Charter::default(),
            &PolicyEngine::default(),
        );
        assert_eq!(decision.outcome, outcome);
        assert_eq!(decision.scope, context.scope);
        assert_eq!(decision.policy_version, 42);
        assert_eq!(decision.reason, "profile default");
    }
}

#[test]
fn shortcuts_are_explicit_and_remembered_rules_can_tighten_them() {
    let request = PolicyRequest::tool("Write");
    for mode in [PermissionMode::Standard, PermissionMode::Workflow] {
        let context = PermissionContext {
            mode,
            local_allowance: true,
            ..context()
        };
        for effect in [PolicyEffect::RequireApproval, PolicyEffect::Deny] {
            let mut charter = Charter::default();
            charter.delegate(PolicyRule::new("standing", PolicyCondition::Always, effect));
            let decision = PermissionPolicyService::evaluate(
                &request,
                &context,
                &charter,
                &PolicyEngine::default(),
            );
            assert_eq!(
                decision.outcome,
                if effect == PolicyEffect::Deny {
                    PermissionOutcome::Deny
                } else {
                    PermissionOutcome::Ask
                }
            );
            assert_eq!(
                decision.source,
                PermissionSource::Charter("standing".into())
            );
        }
    }
    assert_eq!(
        PermissionPolicyService::evaluate(
            &request,
            &context(),
            &Charter::default(),
            &PolicyEngine::default()
        )
        .outcome,
        PermissionOutcome::Ask
    );
    let workflow = PermissionContext {
        mode: PermissionMode::Workflow,
        ..context()
    };
    assert_eq!(
        PermissionPolicyService::evaluate(
            &request,
            &workflow,
            &Charter::default(),
            &PolicyEngine::default()
        )
        .outcome,
        PermissionOutcome::Allow
    );
}

#[test]
fn governance_and_protected_denials_cannot_be_widened() {
    let request = PolicyRequest::tool("Write");
    let mut charter = Charter::default();
    charter.delegate(PolicyRule::new(
        "allow",
        PolicyCondition::Always,
        PolicyEffect::Allow,
    ));
    let governance = PolicyEngine::new(vec![PolicyRule::new(
        "deny",
        PolicyCondition::Always,
        PolicyEffect::Deny,
    )]);
    let decision = PermissionPolicyService::evaluate(&request, &context(), &charter, &governance);
    assert_eq!(decision.outcome, PermissionOutcome::Deny);
    assert_eq!(decision.source, PermissionSource::Governance("deny".into()));
    let protected = PermissionContext {
        protected: true,
        ..context()
    };
    let decision =
        PermissionPolicyService::evaluate(&request, &protected, &charter, &PolicyEngine::default());
    assert_eq!(decision.outcome, PermissionOutcome::Deny);
    assert_eq!(decision.source, PermissionSource::ProtectedPath);
}

#[test]
fn remembered_allow_requires_coverage_of_every_target_but_denies_match_any() {
    let request = PolicyRequest::tool("Write")
        .with_path("src/allowed.rs")
        .with_path("secret/key");
    let mut charter = Charter::default();
    charter.delegate(PolicyRule::new(
        "allow-src",
        PolicyCondition::path("src/*"),
        PolicyEffect::Allow,
    ));
    let decision =
        PermissionPolicyService::evaluate(&request, &context(), &charter, &PolicyEngine::default());
    assert_eq!(decision.outcome, PermissionOutcome::Ask);
    charter.delegate(PolicyRule::new(
        "refuse-src",
        PolicyCondition::path("src/*"),
        PolicyEffect::Deny,
    ));
    let decision =
        PermissionPolicyService::evaluate(&request, &context(), &charter, &PolicyEngine::default());
    assert_eq!(decision.outcome, PermissionOutcome::Deny);
}

#[test]
fn local_denial_survives_shortcuts_and_a_remembered_allow() {
    let context = PermissionContext {
        mode: PermissionMode::Workflow,
        profile_effect: PolicyEffect::Deny,
        local_allowance: true,
        ..context()
    };
    let mut charter = Charter::default();
    charter.delegate(PolicyRule::new(
        "allow",
        PolicyCondition::Always,
        PolicyEffect::Allow,
    ));
    let decision = PermissionPolicyService::evaluate(
        &PolicyRequest::tool("Write"),
        &context,
        &charter,
        &PolicyEngine::default(),
    );
    assert_eq!(decision.outcome, PermissionOutcome::Deny);
    assert_eq!(decision.source, PermissionSource::Profile);
}

#[test]
fn aliases_match_rules_without_becoming_unrelated_targets() {
    let mut request = PolicyRequest::tool("Write").with_path("/repo/src/main.rs");
    request.path_aliases[0].push("src/main.rs".into());
    let mut charter = Charter::default();
    charter.delegate(PolicyRule::new(
        "absolute-allow",
        PolicyCondition::path("/repo/src/*"),
        PolicyEffect::Allow,
    ));
    assert_eq!(
        PermissionPolicyService::evaluate(&request, &context(), &charter, &PolicyEngine::default())
            .outcome,
        PermissionOutcome::Allow
    );
    let governance = PolicyEngine::new(vec![PolicyRule::new(
        "relative-refusal",
        PolicyCondition::path("src/*"),
        PolicyEffect::Deny,
    )]);
    assert_eq!(
        PermissionPolicyService::evaluate(&request, &context(), &charter, &governance).outcome,
        PermissionOutcome::Deny
    );
    request = request.with_path("/repo/other/key");
    assert_eq!(
        PermissionPolicyService::evaluate(&request, &context(), &charter, &PolicyEngine::default())
            .outcome,
        PermissionOutcome::Ask
    );
}

#[test]
fn remembered_warning_cannot_permit_an_unrelated_target() {
    let mut charter = Charter::default();
    charter.delegate(PolicyRule::new(
        "warn-src",
        PolicyCondition::path("src/*"),
        PolicyEffect::Warn,
    ));
    let in_scope = PolicyRequest::tool("Write").with_path("src/allowed.rs");
    assert_eq!(
        PermissionPolicyService::evaluate(
            &in_scope,
            &context(),
            &charter,
            &PolicyEngine::default()
        )
        .outcome,
        PermissionOutcome::Allow
    );
    let mixed = in_scope.with_path("secret/key");
    assert_eq!(
        PermissionPolicyService::evaluate(&mixed, &context(), &charter, &PolicyEngine::default())
            .outcome,
        PermissionOutcome::Ask
    );
}
