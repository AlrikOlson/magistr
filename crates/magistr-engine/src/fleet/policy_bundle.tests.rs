use super::*;
use crate::cost::budget_period::BudgetPeriod;
use crate::cost::budget_scope::BudgetScope;
use crate::policy::policy_condition::PolicyCondition;
use crate::policy::policy_effect::PolicyEffect;
use crate::policy::policy_request::PolicyRequest;
use crate::policy::policy_rule::PolicyRule;

fn org_policy() -> GovernancePolicy {
    GovernancePolicy {
        tool_denylist: vec!["Bash".into()],
        ..GovernancePolicy::default()
    }
}

fn caps() -> Vec<BudgetRule> {
    vec![BudgetRule::new(BudgetScope::Org, BudgetPeriod::Monthly).limit(500.0)]
}

#[test]
fn an_empty_fleet_starts_at_generation_zero_with_nothing_published() {
    let bundle = PolicyBundle::empty();

    assert_eq!(bundle.generation, 0);
    assert!(!bundle.is_published());
    assert!(bundle.verify());
    assert!(bundle.engine().is_empty());
}

#[test]
fn sealing_the_same_contents_twice_gives_the_same_digest() {
    let a = PolicyBundle::seal(7, org_policy(), caps());
    let b = PolicyBundle::seal(7, org_policy(), caps());

    assert_eq!(a.digest, b.digest);
    assert!(a.digest.starts_with("fnv1a64:"));
}

#[test]
fn a_different_generation_of_the_same_policy_digests_differently() {
    let a = PolicyBundle::seal(7, org_policy(), caps());
    let b = PolicyBundle::seal(8, org_policy(), caps());

    assert_ne!(a.digest, b.digest);
}

#[test]
fn changing_the_policy_changes_the_digest() {
    let a = PolicyBundle::seal(7, org_policy(), caps());
    let mut loosened = org_policy();
    loosened.tool_denylist.clear();
    let b = PolicyBundle::seal(7, loosened, caps());

    assert_ne!(a.digest, b.digest);
}

#[test]
fn changing_a_budget_changes_the_digest() {
    let a = PolicyBundle::seal(7, org_policy(), caps());
    let raised = vec![BudgetRule::new(BudgetScope::Org, BudgetPeriod::Monthly).limit(5_000.0)];
    let b = PolicyBundle::seal(7, org_policy(), raised);

    assert_ne!(a.digest, b.digest);
}

#[test]
fn a_bundle_edited_without_resealing_fails_verification() {
    let mut bundle = PolicyBundle::seal(7, org_policy(), caps());
    assert!(bundle.verify());

    // The exact attack the digest is for: drop the denylist, keep the
    // digest and generation, hope nobody checks.
    bundle.policy.tool_denylist.clear();

    assert!(!bundle.verify());
}

#[test]
fn a_non_finite_budget_limit_digests_rather_than_panicking() {
    let nan = vec![BudgetRule::new(BudgetScope::Org, BudgetPeriod::Monthly).limit(f64::NAN)];
    let absent = vec![BudgetRule::new(BudgetScope::Org, BudgetPeriod::Monthly)];

    let sealed = PolicyBundle::seal(3, org_policy(), nan);

    assert!(sealed.digest.starts_with("fnv1a64:"));
    assert!(sealed.verify());
    // serde_json writes a non-finite float as `null`, so the guard that
    // matters is that a NaN cap does not collide with an absent one.
    assert_ne!(
        sealed.digest,
        PolicyBundle::seal(3, org_policy(), absent).digest
    );
}

#[test]
fn a_higher_generation_supersedes_a_lower_one() {
    let old = PolicyBundle::seal(4, org_policy(), caps());
    let new = PolicyBundle::seal(5, org_policy(), caps());

    assert!(new.supersedes(&old));
    assert!(!old.supersedes(&new));
    assert!(!new.supersedes(&new));
}

#[test]
fn applying_a_bundle_over_a_local_policy_keeps_both_rule_sets() {
    let bundle = PolicyBundle::seal(1, org_policy(), vec![]);
    let local = GovernancePolicy {
        tool_denylist: vec!["WebFetch".into()],
        ..GovernancePolicy::default()
    };

    let merged = bundle.merge_local(&local);

    assert_eq!(merged.len(), bundle.engine().len() + local.compile().len());
}

#[test]
fn a_local_policy_cannot_loosen_what_the_bundle_denies() {
    let bundle = PolicyBundle::seal(1, org_policy(), vec![]);
    // A local roster that tries to bless the very tool the org denies.
    let local = GovernancePolicy {
        rules: vec![PolicyRule::new(
            "local-allow-bash",
            PolicyCondition::tool("Bash"),
            PolicyEffect::Allow,
        )],
        ..GovernancePolicy::default()
    };

    let decision = bundle
        .merge_local(&local)
        .evaluate(&PolicyRequest::tool("Bash"));

    assert_eq!(decision.effect, PolicyEffect::Deny);
    assert_eq!(decision.binding.as_deref(), Some("tool-denylist:Bash"));
}

#[test]
fn a_local_policy_can_deny_what_the_bundle_permits() {
    let bundle = PolicyBundle::seal(1, GovernancePolicy::default(), vec![]);
    let local = GovernancePolicy {
        tool_denylist: vec!["WebFetch".into()],
        ..GovernancePolicy::default()
    };

    let decision = bundle
        .merge_local(&local)
        .evaluate(&PolicyRequest::tool("WebFetch"));

    assert_eq!(decision.effect, PolicyEffect::Deny);
}

#[test]
fn a_bundle_round_trips_through_json_with_its_seal_intact() {
    let bundle = PolicyBundle::seal(12, org_policy(), caps());

    let json = serde_json::to_string(&bundle).expect("serializable");
    let back: PolicyBundle = serde_json::from_str(&json).expect("deserializable");

    assert_eq!(back, bundle);
    assert!(back.verify());
}
