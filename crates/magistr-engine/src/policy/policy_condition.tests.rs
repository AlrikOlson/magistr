use super::*;

fn bash(command: &str) -> PolicyRequest {
    PolicyRequest::tool("Bash").with_command(command)
}

#[test]
fn always_matches_even_an_empty_request() {
    assert!(PolicyCondition::Always.matches(&PolicyRequest::new()));
}

#[test]
fn a_leaf_over_an_absent_field_is_false_not_true() {
    let empty = PolicyRequest::new();
    assert!(!PolicyCondition::tool("*").matches(&empty));
    assert!(!PolicyCondition::command("*").matches(&empty));
    assert!(!PolicyCondition::path("*").matches(&empty));
    assert!(!PolicyCondition::model("*").matches(&empty));
    assert!(!PolicyCondition::gate("*").matches(&empty));
    assert!(!PolicyCondition::risk_at_least(RiskTier::Low).matches(&empty));
}

#[test]
fn tool_and_command_match_by_glob() {
    assert!(PolicyCondition::tool("Bash").matches(&bash("ls")));
    assert!(PolicyCondition::tool("mcp__*").matches(&PolicyRequest::tool("mcp__magistr__gate")));
    assert!(PolicyCondition::command("*rm -rf*").matches(&bash("sudo rm -rf /tmp")));
    assert!(!PolicyCondition::command("*rm -rf*").matches(&bash("ls -la")));
}

#[test]
fn path_matches_when_any_touched_path_matches() {
    let request = PolicyRequest::tool("Write")
        .with_path("src/main.rs")
        .with_path(".magistr/team.json");
    assert!(PolicyCondition::path("*team.json").matches(&request));
    assert!(PolicyCondition::path("src/*.rs").matches(&request));
    assert!(!PolicyCondition::path("*.toml").matches(&request));
}

#[test]
fn risk_at_least_is_inclusive_at_its_own_tier() {
    let medium = PolicyRequest::tool("Write").with_risk(RiskTier::Medium);
    assert!(PolicyCondition::risk_at_least(RiskTier::Low).matches(&medium));
    assert!(PolicyCondition::risk_at_least(RiskTier::Medium).matches(&medium));
    assert!(!PolicyCondition::risk_at_least(RiskTier::High).matches(&medium));
}

#[test]
fn combinators_compose_the_leaves() {
    let request = bash("git push --force");
    let condition = PolicyCondition::all_of(vec![
        PolicyCondition::tool("Bash"),
        PolicyCondition::any_of(vec![
            PolicyCondition::command("*--force*"),
            PolicyCondition::command("*rm -rf*"),
        ]),
        PolicyCondition::negate(PolicyCondition::command("*--dry-run*")),
    ]);
    assert!(condition.matches(&request));
    assert!(!condition.matches(&bash("git push --force --dry-run")));
}

#[test]
fn empty_combinators_take_the_neutral_element() {
    let request = bash("ls");
    assert!(!PolicyCondition::any_of(Vec::new()).matches(&request));
    assert!(PolicyCondition::all_of(Vec::new()).matches(&request));
}

#[test]
fn round_trips_through_the_roster_json() {
    let condition = PolicyCondition::all_of(vec![
        PolicyCondition::tool("Bash"),
        PolicyCondition::negate(PolicyCondition::command("cargo *")),
        PolicyCondition::risk_at_least(RiskTier::High),
    ]);
    let json = serde_json::to_string(&condition).expect("serialize");
    let back: PolicyCondition = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(back, condition);
    assert!(json.contains("\"all_of\""));
    assert!(json.contains("\"risk_at_least\""));
}
