use super::*;

#[test]
fn new_tracker_starts_at_zero() {
    let t = CostTracker::new(BudgetConfig::none());
    assert_eq!(t.total_cost(), 0.0);
    assert_eq!(t.total_turns(), 0);
    assert!(t.turn_costs().is_empty());
}

#[test]
fn record_turn_accumulates() {
    let mut t = CostTracker::new(BudgetConfig::none());
    t.record_turn(1.0, 5000, None);
    t.record_turn(2.0, 3000, None);

    assert!((t.total_cost() - 3.0).abs() < f64::EPSILON);
    assert_eq!(t.total_turns(), 2);
    assert_eq!(t.turn_costs(), &[1.0, 2.0]);
}

#[test]
fn record_turn_accumulates_tokens() {
    let mut t = CostTracker::new(BudgetConfig::none());
    let tokens = TokenBreakdown {
        input_tokens: 100,
        output_tokens: 50,
        ..Default::default()
    };

    t.record_turn(0.5, 1000, Some(&tokens));
    t.record_turn(0.5, 1000, Some(&tokens));

    assert_eq!(t.total_tokens().input_tokens, 200);
    assert_eq!(t.total_tokens().output_tokens, 100);
}

#[test]
fn budget_ok_when_under_limit() {
    let mut t = CostTracker::new(BudgetConfig::with_limit(5.0));
    let status = t.record_turn(2.0, 1000, None);
    assert_eq!(status, BudgetStatus::Ok);
}

#[test]
fn budget_exceeded_when_over_limit() {
    let mut t = CostTracker::new(BudgetConfig::with_limit(5.0));
    t.record_turn(3.0, 1000, None);
    let status = t.record_turn(3.0, 1000, None);
    assert!(status.should_stop());
}

#[test]
fn budget_warning_issued_once() {
    let mut t = CostTracker::new(BudgetConfig::with_limit_and_warn(5.0, 3.0));

    // First turn: under warning
    let s1 = t.record_turn(2.0, 1000, None);
    assert_eq!(s1, BudgetStatus::Ok);

    // Second turn: crosses warning
    let s2 = t.record_turn(1.5, 1000, None);
    assert!(s2.should_warn());

    // Third turn: still in warning zone, but already issued
    let s3 = t.record_turn(0.3, 1000, None);
    assert_eq!(s3, BudgetStatus::Ok);
}

#[test]
fn to_cost_entry_captures_state() {
    let mut t = CostTracker::new(BudgetConfig::none());
    let tokens = TokenBreakdown {
        input_tokens: 500,
        output_tokens: 200,
        ..Default::default()
    };
    t.record_turn(1.23, 5000, Some(&tokens));
    t.record_turn(0.77, 3000, None);

    let entry = t.to_cost_entry("sess-abc", "my-project");
    assert_eq!(entry.session_id, "sess-abc");
    assert_eq!(entry.project, "my-project");
    assert!((entry.cost_usd - 2.0).abs() < f64::EPSILON);
    assert_eq!(entry.turns, 2);
    assert_eq!(entry.duration_ms, 8000);
    assert_eq!(entry.tokens.input_tokens, 500);
}

#[test]
fn cost_display_formatting() {
    let mut t = CostTracker::new(BudgetConfig::none());
    assert_eq!(t.cost_display(), "$0.0000");

    t.record_turn(0.005, 1000, None);
    assert_eq!(t.cost_display(), "$0.0050");

    t.record_turn(5.0, 1000, None);
    assert_eq!(t.cost_display(), "$5.00");
}

#[test]
fn a_tracker_stamps_its_actor_and_model_onto_the_entry() {
    let mut t = CostTracker::new(BudgetConfig::none())
        .with_actor("ada@example.com")
        .with_model("claude-opus-5");
    t.record_turn(0.5, 1000, None);

    assert_eq!(t.actor(), Some("ada@example.com"));
    assert_eq!(t.model(), Some("claude-opus-5"));

    let entry = t.to_cost_entry("sess-1", "magistr");
    assert_eq!(entry.actor.as_deref(), Some("ada@example.com"));
    assert_eq!(entry.model.as_deref(), Some("claude-opus-5"));
}

#[test]
fn an_unattributed_tracker_writes_an_unattributed_entry() {
    let t = CostTracker::new(BudgetConfig::none());
    let entry = t.to_cost_entry("sess-1", "magistr");
    assert_eq!(entry.actor, None);
    assert_eq!(entry.model, None);
}

#[test]
fn the_model_can_arrive_after_the_session_starts() {
    let mut t = CostTracker::new(BudgetConfig::none());
    t.record_turn(0.25, 500, None);
    t.set_model("claude-sonnet-5");
    assert_eq!(
        t.to_cost_entry("s", "p").model.as_deref(),
        Some("claude-sonnet-5")
    );
}
