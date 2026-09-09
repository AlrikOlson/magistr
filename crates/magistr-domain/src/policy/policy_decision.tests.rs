use alloc::vec;

use super::*;

use crate::policy::PolicyCondition;

fn rule(id: &str, effect: PolicyEffect) -> PolicyRule {
    PolicyRule::new(id, PolicyCondition::Always, effect)
}

#[test]
fn no_matches_is_unconstrained() {
    let decision = PolicyDecision::from_matches(Vec::<&PolicyRule>::new());
    assert_eq!(decision, PolicyDecision::unconstrained());
    assert_eq!(decision.effect, PolicyEffect::Allow);
    assert!(!decision.is_violation());
    assert_eq!(decision.explain(), "no policy rule applies");
}

#[test]
fn the_effect_is_the_maximum_not_the_last_match() {
    let rules = vec![
        rule("a", PolicyEffect::Deny),
        rule("b", PolicyEffect::Warn),
        rule("c", PolicyEffect::Allow),
    ];
    let decision = PolicyDecision::from_matches(&rules);
    assert_eq!(decision.effect, PolicyEffect::Deny);
    assert_eq!(decision.binding.as_deref(), Some("a"));
    assert_eq!(decision.matched, vec!["a", "b", "c"]);
}

#[test]
fn rule_order_cannot_loosen_the_outcome() {
    let forwards = vec![
        rule("warn", PolicyEffect::Warn),
        rule("deny", PolicyEffect::Deny),
    ];
    let backwards = vec![
        rule("deny", PolicyEffect::Deny),
        rule("warn", PolicyEffect::Warn),
    ];
    assert_eq!(
        PolicyDecision::from_matches(&forwards).effect,
        PolicyDecision::from_matches(&backwards).effect
    );
}

#[test]
fn an_override_needs_every_rule_at_the_binding_height_to_allow_it() {
    let both = vec![
        rule("a", PolicyEffect::Deny).overridable(),
        rule("b", PolicyEffect::Deny).overridable(),
    ];
    assert!(PolicyDecision::from_matches(&both).override_allowed);

    let one_locked = vec![
        rule("a", PolicyEffect::Deny).overridable(),
        rule("b", PolicyEffect::Deny),
    ];
    assert!(!PolicyDecision::from_matches(&one_locked).override_allowed);
}

#[test]
fn a_laxer_overridable_rule_does_not_unlock_a_stricter_one() {
    let rules = vec![
        rule("hard-deny", PolicyEffect::Deny),
        rule("soft-warn", PolicyEffect::Warn).overridable(),
    ];
    let decision = PolicyDecision::from_matches(&rules);
    assert_eq!(decision.effect, PolicyEffect::Deny);
    assert!(!decision.override_allowed);
}

#[test]
fn an_allow_only_decision_is_never_overridable() {
    let rules = vec![rule("a", PolicyEffect::Allow).overridable()];
    let decision = PolicyDecision::from_matches(&rules);
    assert_eq!(decision.effect, PolicyEffect::Allow);
    assert!(!decision.override_allowed);
}

#[test]
fn explain_names_the_binding_rule() {
    let rules = vec![rule("no-force-push", PolicyEffect::Deny)];
    assert_eq!(
        PolicyDecision::from_matches(&rules).explain(),
        "deny by policy rule 'no-force-push'"
    );
}
