use super::*;

use core::str::FromStr;

#[test]
fn effects_order_from_permissive_to_restrictive() {
    assert!(PolicyEffect::Allow < PolicyEffect::Warn);
    assert!(PolicyEffect::Warn < PolicyEffect::RequireApproval);
    assert!(PolicyEffect::RequireApproval < PolicyEffect::Deny);
}

#[test]
fn tighten_takes_the_stricter_side_whichever_way_round() {
    assert_eq!(
        PolicyEffect::Allow.tighten(PolicyEffect::Deny),
        PolicyEffect::Deny
    );
    assert_eq!(
        PolicyEffect::Deny.tighten(PolicyEffect::Allow),
        PolicyEffect::Deny
    );
}

#[test]
fn only_allow_is_not_a_violation() {
    assert!(!PolicyEffect::Allow.is_violation());
    for effect in PolicyEffect::ALL.iter().skip(1) {
        assert!(effect.is_violation(), "{effect} should be a violation");
    }
}

#[test]
fn round_trips_through_its_string_form() {
    for effect in PolicyEffect::ALL {
        assert_eq!(PolicyEffect::from_str(effect.as_str()), Ok(effect));
    }
    assert_eq!(PolicyEffect::from_str("BLOCK"), Ok(PolicyEffect::Deny));
    assert_eq!(
        PolicyEffect::from_str("require-approval"),
        Ok(PolicyEffect::RequireApproval)
    );
    assert!(PolicyEffect::from_str("maybe").is_err());
}

#[test]
fn serializes_snake_case_for_the_roster_file() {
    let json = serde_json::to_string(&PolicyEffect::RequireApproval).expect("serialize");
    assert_eq!(json, "\"require_approval\"");
}
