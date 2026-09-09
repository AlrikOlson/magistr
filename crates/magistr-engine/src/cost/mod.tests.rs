use super::budget_status::check_budget;
use super::*;

// ── BudgetConfig tests ──────────────────────────────────────────────

#[test]
fn budget_config_none() {
    let b = BudgetConfig::none();
    assert!(!b.is_set());
    assert_eq!(b.limit, None);
    assert_eq!(b.warn_threshold, None);
}

#[test]
fn budget_config_with_limit() {
    let b = BudgetConfig::with_limit(5.0);
    assert!(b.is_set());
    assert_eq!(b.limit, Some(5.0));
}

#[test]
fn budget_config_with_limit_and_warn() {
    let b = BudgetConfig::with_limit_and_warn(5.0, 3.0);
    assert!(b.is_set());
    assert_eq!(b.limit, Some(5.0));
    assert_eq!(b.warn_threshold, Some(3.0));
}

#[test]
fn budget_config_builder_chain() {
    let b = BudgetConfig::none().limit(5.0).warn_threshold(3.0);
    assert!(b.is_set());
    assert_eq!(b.limit, Some(5.0));
    assert_eq!(b.warn_threshold, Some(3.0));
}

#[test]
fn budget_config_builder_limit_only() {
    let b = BudgetConfig::none().limit(10.0);
    assert!(b.is_set());
    assert_eq!(b.limit, Some(10.0));
    assert_eq!(b.warn_threshold, None);
}

#[test]
fn budget_config_default_is_none() {
    let b = BudgetConfig::default();
    assert!(!b.is_set());
}

// ── BudgetStatus tests ──────────────────────────────────────────────

#[test]
fn check_budget_no_budget_always_ok() {
    assert_eq!(check_budget(100.0, &BudgetConfig::none()), BudgetStatus::Ok);
}

#[test]
fn check_budget_under_limit() {
    let status = check_budget(3.0, &BudgetConfig::with_limit(5.0));
    assert_eq!(status, BudgetStatus::Ok);
    assert!(!status.should_stop());
}

#[test]
fn check_budget_at_limit_is_exceeded() {
    let status = check_budget(5.0, &BudgetConfig::with_limit(5.0));
    assert!(status.should_stop());
}

#[test]
fn check_budget_over_limit() {
    let status = check_budget(7.5, &BudgetConfig::with_limit(5.0));
    assert!(matches!(
        status,
        BudgetStatus::Exceeded { overage, limit: 5.0 }
        if (overage - 2.5).abs() < f64::EPSILON
    ));
}

#[test]
fn check_budget_warning_triggers() {
    let config = BudgetConfig::with_limit_and_warn(5.0, 3.0);
    let status = check_budget(3.5, &config);
    assert!(status.should_warn());
    assert!(!status.should_stop());
}

#[test]
fn check_budget_exceeded_beats_warning() {
    let config = BudgetConfig::with_limit_and_warn(5.0, 3.0);
    let status = check_budget(6.0, &config);
    assert!(matches!(status, BudgetStatus::Exceeded { .. }));
}

// ── CostGovernor tests ──────────────────────────────────────────────

#[test]
fn new_governor_starts_at_zero() {
    let gov = CostGovernor::new(BudgetConfig::with_limit(5.0));
    assert_eq!(gov.total_cost_usd(), 0.0);
    assert_eq!(gov.cost().turns, 0);
    assert!(!gov.is_exceeded());
}

#[test]
fn record_turn_accumulates_cost() {
    let mut gov = CostGovernor::new(BudgetConfig::none());
    let events = gov.record_turn(1.50, 100, 50);

    assert_eq!(gov.total_cost_usd(), 1.50);
    assert_eq!(gov.cost().turns, 1);
    assert_eq!(gov.cost().input_tokens, 100);
    assert_eq!(gov.cost().output_tokens, 50);

    // Should emit CostUpdated (a priced turn is also sampled — see
    // cost_governor.tests.rs; asserted by variant, not by position, so
    // adding a channel payload does not break the total's contract).
    assert!(events.iter().any(|e| matches!(
        e,
        CostEvent::CostUpdated { total_cost_usd, turn_cost_usd, turns }
        if (*total_cost_usd - 1.5).abs() < f64::EPSILON
            && (*turn_cost_usd - 1.5).abs() < f64::EPSILON
            && *turns == 1
    )));
}

#[test]
fn record_turn_emits_warning_once() {
    let mut gov = CostGovernor::new(BudgetConfig::with_limit_and_warn(5.0, 2.0));

    let warnings = |events: &[CostEvent]| {
        events
            .iter()
            .filter(|e| matches!(e, CostEvent::BudgetWarning { .. }))
            .count()
    };

    // Turn 1: under warning
    let events = gov.record_turn(1.0, 50, 25);
    assert_eq!(warnings(&events), 0);

    // Turn 2: crosses warning threshold
    let events = gov.record_turn(1.5, 50, 25);
    assert_eq!(warnings(&events), 1);

    // Turn 3: still above warning, but warning already emitted
    let events = gov.record_turn(0.5, 50, 25);
    assert_eq!(warnings(&events), 0);
}

#[test]
fn record_turn_emits_exceeded() {
    let mut gov = CostGovernor::new(BudgetConfig::with_limit(2.0));

    let events = gov.record_turn(2.5, 100, 50);
    assert!(
        events
            .iter()
            .any(|e| matches!(e, CostEvent::BudgetExceeded { .. }))
    );
    assert!(gov.is_exceeded());
}

#[test]
fn record_turn_exceeded_every_time() {
    let mut gov = CostGovernor::new(BudgetConfig::with_limit(1.0));

    gov.record_turn(1.5, 50, 25);
    let events = gov.record_turn(0.5, 50, 25);
    // Should still emit exceeded on subsequent turns
    assert!(
        events
            .iter()
            .any(|e| matches!(e, CostEvent::BudgetExceeded { .. }))
    );
}

#[test]
fn extend_budget_returns_event() {
    let mut gov = CostGovernor::new(BudgetConfig::with_limit(5.0));
    let event = gov.extend_budget(10.0);
    assert!(matches!(
        event,
        Some(CostEvent::BudgetExtended {
            old_limit: 5.0,
            new_limit: 10.0
        })
    ));
    assert_eq!(gov.budget().limit, Some(10.0));
}

#[test]
fn extend_budget_no_limit_returns_none() {
    let mut gov = CostGovernor::new(BudgetConfig::none());
    assert!(gov.extend_budget(10.0).is_none());
}

#[test]
fn extend_budget_resets_warning_when_below_threshold() {
    // Warn at 5.0, limit at 6.0. Spend 5.5 → triggers warning.
    let mut gov = CostGovernor::new(BudgetConfig::with_limit_and_warn(6.0, 5.0));
    let events = gov.record_turn(5.5, 50, 25);
    assert!(
        events
            .iter()
            .any(|e| matches!(e, CostEvent::BudgetWarning { .. }))
    );

    // Extend limit to 20.0 AND raise warn threshold above current spending
    // by replacing budget config entirely via extend_budget.
    // extend_budget only changes the limit, not the warn threshold.
    // Since spending (5.5) is still >= warn (5.0), warning_emitted stays true.
    gov.extend_budget(20.0);

    // Next turn: warning already emitted and spending still above threshold,
    // so no duplicate warning.
    let events = gov.record_turn(0.0, 0, 0);
    assert!(
        !events
            .iter()
            .any(|e| matches!(e, CostEvent::BudgetWarning { .. }))
    );
}

#[test]
fn extend_budget_does_not_reemit_when_still_above_warn() {
    let mut gov = CostGovernor::new(BudgetConfig::with_limit_and_warn(5.0, 3.0));
    gov.record_turn(3.5, 50, 25); // triggers warning
    gov.extend_budget(10.0); // raise limit, but spending still above warn (3.0)

    // Warning was already emitted and spending is still above threshold
    let events = gov.record_turn(0.0, 0, 0);
    assert!(
        !events
            .iter()
            .any(|e| matches!(e, CostEvent::BudgetWarning { .. }))
    );
}

#[test]
fn cost_event_serialization_roundtrip() {
    let event = CostEvent::CostUpdated {
        total_cost_usd: 3.50,
        turn_cost_usd: 1.25,
        turns: 3,
    };
    let json = serde_json::to_string(&event).expect("serialize");
    let parsed: CostEvent = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(event, parsed);
}

#[test]
fn governor_serialization_roundtrip() {
    let mut gov = CostGovernor::new(BudgetConfig::with_limit(5.0));
    gov.record_turn(1.5, 100, 50);

    let json = serde_json::to_string(&gov).expect("serialize");
    let parsed: CostGovernor = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(gov, parsed);
}
