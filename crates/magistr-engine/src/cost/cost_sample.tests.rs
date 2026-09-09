use super::*;
use crate::cost::BudgetScope;

#[test]
fn new_sample_starts_with_the_turn_as_the_whole_total() {
    let sample = CostSample::new(0.25, 1_000, 200);
    assert_eq!(sample.turn_cost_usd, 0.25);
    assert_eq!(sample.total_cost_usd, 0.25);
    assert_eq!(sample.tokens(TokenType::Input), 1_000);
    assert_eq!(sample.tokens(TokenType::Output), 200);
}

#[test]
fn total_is_carried_separately_from_the_turn() {
    let sample = CostSample::new(0.25, 10, 5).with_total(3.75);
    assert_eq!(sample.turn_cost_usd, 0.25);
    assert_eq!(sample.total_cost_usd, 3.75);
}

#[test]
fn operation_defaults_to_the_agent_invocation_the_convention_names() {
    assert_eq!(CostSample::default().operation_label(), "invoke_agent");
    assert_eq!(
        CostSample::default()
            .with_operation("chat")
            .operation_label(),
        "chat"
    );
    assert_eq!(
        CostSample::default().with_operation("  ").operation_label(),
        "invoke_agent"
    );
}

#[test]
fn a_turn_with_no_spend_and_no_tokens_is_empty() {
    assert!(CostSample::default().is_empty());
    assert!(!CostSample::new(0.0, 1, 0).is_empty());
    assert!(!CostSample::new(0.01, 0, 0).is_empty());
}

#[test]
fn attribution_rides_along_and_round_trips() {
    let sample = CostSample::new(0.5, 100, 20)
        .with_attribution(
            CostAttribution::default()
                .with_actor("dev@example.com")
                .with_model("claude-opus-5")
                .with_scope(BudgetScope::Project),
        )
        .with_duration_s(12.5);
    let json = serde_json::to_string(&sample).expect("serializes");
    let back: CostSample = serde_json::from_str(&json).expect("deserializes");
    assert_eq!(back, sample);
    assert_eq!(back.attribution.scope_label(), "project");
    assert_eq!(back.duration_s, 12.5);
}
