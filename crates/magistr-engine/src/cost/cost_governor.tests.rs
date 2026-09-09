use super::*;
use crate::cost::{BudgetScope, TokenType};

fn attributed() -> CostAttribution {
    CostAttribution::default()
        .with_actor("dev@example.com")
        .with_model("claude-opus-5")
        .with_scope(BudgetScope::Team)
}

#[test]
fn a_priced_turn_emits_an_attributed_sample_beside_the_total() {
    let mut governor = CostGovernor::new(BudgetConfig::none()).with_attribution(attributed());
    let events = governor.record_turn(0.5, 1_200, 300);

    let sample = events
        .iter()
        .find_map(|e| match e {
            CostEvent::CostSampled { sample } => Some(sample),
            _ => None,
        })
        .expect("a priced turn is sampled");

    assert_eq!(sample.turn_cost_usd, 0.5);
    assert_eq!(sample.total_cost_usd, 0.5);
    assert_eq!(sample.tokens(TokenType::Input), 1_200);
    assert_eq!(sample.tokens(TokenType::Output), 300);
    assert_eq!(sample.attribution.actor_label(), "dev@example.com");
    assert_eq!(sample.attribution.model_label(), "claude-opus-5");
    assert_eq!(sample.attribution.scope_label(), "team");
}

#[test]
fn the_sample_carries_the_running_total_not_just_the_turn() {
    let mut governor = CostGovernor::new(BudgetConfig::none());
    governor.record_turn(1.0, 10, 10);
    let events = governor.record_turn(0.25, 10, 10);

    let sample = events
        .iter()
        .find_map(|e| match e {
            CostEvent::CostSampled { sample } => Some(sample),
            _ => None,
        })
        .expect("sampled");
    assert_eq!(sample.turn_cost_usd, 0.25);
    assert_eq!(sample.total_cost_usd, 1.25);
}

#[test]
fn an_empty_turn_is_not_sampled() {
    let mut governor = CostGovernor::new(BudgetConfig::none());
    let events = governor.record_turn(0.0, 0, 0);
    assert!(
        !events
            .iter()
            .any(|e| matches!(e, CostEvent::CostSampled { .. })),
        "a turn with no spend and no tokens adds no series"
    );
    assert!(
        events
            .iter()
            .any(|e| matches!(e, CostEvent::CostUpdated { .. })),
        "the running total is still reported"
    );
}

#[test]
fn attribution_can_be_set_once_the_agent_reports_the_model() {
    let mut governor = CostGovernor::new(BudgetConfig::none());
    assert_eq!(governor.attribution().model_label(), "unknown");
    governor.set_attribution(attributed());
    assert_eq!(governor.attribution().model_label(), "claude-opus-5");
}
