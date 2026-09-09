use super::*;

#[test]
fn a_rule_matches_through_its_condition() {
    let rule = PolicyRule::new(
        "no-force-push",
        PolicyCondition::command("*push --force*"),
        PolicyEffect::Deny,
    );
    assert!(rule.matches(&PolicyRequest::tool("Bash").with_command("git push --force")));
    assert!(!rule.matches(&PolicyRequest::tool("Bash").with_command("git push")));
}

#[test]
fn rules_are_not_overridable_unless_they_say_so() {
    let rule = PolicyRule::new("locked", PolicyCondition::Always, PolicyEffect::Deny);
    assert!(!rule.override_allowed);
    assert!(rule.overridable().override_allowed);
}

#[test]
fn round_trips_through_the_roster_json() {
    let rule = PolicyRule::new(
        "protect-roster",
        PolicyCondition::path("*team.json"),
        PolicyEffect::RequireApproval,
    )
    .describe("the roster is an org artifact")
    .overridable();
    let json = serde_json::to_string(&rule).expect("serialize");
    let back: PolicyRule = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(back, rule);
}

#[test]
fn description_and_override_default_when_the_roster_omits_them() {
    let json = r#"{"id":"minimal","condition":{"type":"always"},"effect":"warn"}"#;
    let rule: PolicyRule = serde_json::from_str(json).expect("deserialize");
    assert_eq!(rule.description, "");
    assert!(!rule.override_allowed);
    assert_eq!(rule.effect, PolicyEffect::Warn);
}
