use super::*;

#[test]
fn new_computes_overage() {
    let screen = BudgetExceededScreen::new(5.0, 7.50, 10, &TokenBreakdown::default(), &[]);
    assert!((screen.overage - 2.50).abs() < f64::EPSILON);
}

#[test]
fn new_computes_avg_cost_per_turn() {
    let screen =
        BudgetExceededScreen::new(5.0, 6.0, 3, &TokenBreakdown::default(), &[1.0, 2.0, 3.0]);
    assert!((screen.avg_cost_per_turn - 2.0).abs() < f64::EPSILON);
}

#[test]
fn new_zero_turns_no_panic() {
    let screen = BudgetExceededScreen::new(5.0, 0.0, 0, &TokenBreakdown::default(), &[]);
    assert!((screen.avg_cost_per_turn).abs() < f64::EPSILON);
}

#[test]
fn suggested_extension_is_double() {
    let screen = BudgetExceededScreen::new(5.0, 5.50, 5, &TokenBreakdown::default(), &[]);
    assert!((screen.suggested_extension - 10.0).abs() < f64::EPSILON);
}

#[test]
fn overage_clamped_to_zero_when_under() {
    let screen = BudgetExceededScreen::new(10.0, 5.0, 5, &TokenBreakdown::default(), &[]);
    assert!((screen.overage).abs() < f64::EPSILON);
}

#[test]
fn summary_contains_key_info() {
    let tokens = TokenBreakdown {
        input_tokens: 5000,
        output_tokens: 2000,
        cache_read_tokens: 100,
        cache_write_tokens: 50,
    };
    let screen = BudgetExceededScreen::new(5.0, 6.25, 4, &tokens, &[1.0, 1.5, 2.0, 1.75]);
    let text = screen.summary();

    assert!(text.contains("Budget Exceeded"));
    assert!(text.contains("$5.00"));
    assert!(text.contains("$6.25"));
    assert!(text.contains("+$1.25 over"));
    assert!(text.contains("Turns:       4"));
    assert!(text.contains("5000 input"));
    assert!(text.contains("2000 output"));
    assert!(text.contains("100 read"));
    assert!(text.contains("50 write"));
    assert!(text.contains("--budget 10.00"));
    assert!(text.contains("--continue"));
}

#[test]
fn summary_omits_cache_when_zero() {
    let screen = BudgetExceededScreen::new(
        5.0,
        5.50,
        2,
        &TokenBreakdown {
            input_tokens: 1000,
            output_tokens: 500,
            cache_read_tokens: 0,
            cache_write_tokens: 0,
        },
        &[],
    );
    let text = screen.summary();
    assert!(!text.contains("Cache:"));
}

#[test]
fn from_summary_matches_direct() {
    let tokens = TokenBreakdown {
        input_tokens: 1000,
        output_tokens: 500,
        ..Default::default()
    };
    let summary = CostSummary {
        total_cost: 7.50,
        total_turns: 5,
        entry_count: 1,
        earliest: None,
        latest: None,
        total_tokens: tokens.clone(),
    };
    let turn_costs = vec![1.0, 1.5, 2.0, 1.5, 1.5];

    let screen = BudgetExceededScreen::from_summary(5.0, &summary, &turn_costs);
    assert!((screen.total_spent - 7.50).abs() < f64::EPSILON);
    assert_eq!(screen.turns_completed, 5);
    assert_eq!(screen.tokens, tokens);
    assert_eq!(screen.turn_costs.len(), 5);
}
