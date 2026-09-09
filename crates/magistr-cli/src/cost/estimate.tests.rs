use super::*;

#[test]
fn estimate_display_small_costs() {
    let est = CostEstimate {
        low: 0.003,
        high: 0.008,
        estimated_turns: 2,
        avg_cost_per_turn: 0.005,
        sample_size: 5,
    };
    let display = est.display();
    assert!(display.contains("$0.0030"));
    assert!(display.contains("$0.0080"));
}

#[test]
fn estimate_display_normal_costs() {
    let est = CostEstimate {
        low: 1.50,
        high: 4.00,
        estimated_turns: 5,
        avg_cost_per_turn: 0.55,
        sample_size: 10,
    };
    let display = est.display();
    assert!(display.contains("$1.50"));
    assert!(display.contains("$4.00"));
    assert!(display.contains("~5 turns"));
}
