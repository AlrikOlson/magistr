use super::*;
use crate::cost::cost_entry::CostEntry;

use crate::cost::types::TokenBreakdown;
use chrono::Utc;

fn sample_entry(cost: f64, turns: u32) -> CostEntry {
    CostEntry {
        session_id: format!("s-{cost}"),
        project: "test".into(),
        timestamp: Utc::now(),
        cost_usd: cost,
        tokens: TokenBreakdown::default(),
        turns,
        duration_ms: 5000,
        actor: None,
        model: None,
    }
}

#[test]
fn estimate_returns_none_with_insufficient_data() {
    let history = vec![sample_entry(1.0, 2), sample_entry(2.0, 3)];
    assert!(estimate_cost(&history, None).is_none());
}

#[test]
fn estimate_returns_none_for_empty_history() {
    assert!(estimate_cost(&[], None).is_none());
}

#[test]
fn estimate_produces_range() {
    let history = vec![
        sample_entry(1.0, 2), // $0.50/turn
        sample_entry(3.0, 3), // $1.00/turn
        sample_entry(6.0, 4), // $1.50/turn
        sample_entry(2.0, 2), // $1.00/turn
    ];

    let est = estimate_cost(&history, Some(3)).unwrap();
    assert_eq!(est.estimated_turns, 3);
    assert!(est.low > 0.0);
    assert!(est.high >= est.low);
    assert_eq!(est.sample_size, 4);
}

#[test]
fn estimate_uses_historical_turns_when_none_provided() {
    let history = vec![
        sample_entry(1.0, 4),
        sample_entry(2.0, 6),
        sample_entry(3.0, 5),
    ];

    let est = estimate_cost(&history, None).unwrap();
    // Average turns: (4+6+5)/3 = 5
    assert_eq!(est.estimated_turns, 5);
}

#[test]
fn percentile_single_value() {
    assert!((percentile(&[5.0], 50) - 5.0).abs() < f64::EPSILON);
}

#[test]
fn percentile_two_values() {
    let vals = vec![2.0, 8.0];
    let p50 = percentile(&vals, 50);
    assert!((p50 - 5.0).abs() < f64::EPSILON);
}

#[test]
fn percentile_empty() {
    assert!((percentile(&[], 50) - 0.0).abs() < f64::EPSILON);
}

#[test]
fn percentile_quartiles() {
    let vals = vec![1.0, 2.0, 3.0, 4.0, 5.0];
    let p25 = percentile(&vals, 25);
    let p75 = percentile(&vals, 75);
    assert!((p25 - 2.0).abs() < f64::EPSILON);
    assert!((p75 - 4.0).abs() < f64::EPSILON);
}

#[test]
fn estimate_skips_zero_turn_sessions() {
    let history = vec![
        sample_entry(1.0, 2),
        sample_entry(0.0, 0), // Would cause div-by-zero
        sample_entry(2.0, 3),
        sample_entry(3.0, 4),
    ];

    let est = estimate_cost(&history, Some(3)).unwrap();
    // Only 3 entries used (the zero-turn one is filtered)
    assert!(est.avg_cost_per_turn > 0.0);
}
