use super::*;
use crate::cost::csv_export::to_csv;
use crate::cost::estimate_cost::estimate_cost;
use serde_json::json;

fn entry(price: serde_json::Value) -> CostEntry {
    serde_json::from_value(json!({
        "session_id": "unpriced",
        "project": "project",
        "timestamp": "2026-10-13T00:00:00Z",
        "cost_usd": price,
        "turns": 1,
        "duration_ms": 100
    }))
    .expect("current ledger accepts unreported spending")
}

#[test]
fn unreported_price_round_trips_and_never_displays_as_free() {
    let unknown = entry(serde_json::Value::Null);
    assert!(serde_json::to_value(&unknown).unwrap()["cost_usd"].is_null());
    let summary = CostSummary::from_entries(std::slice::from_ref(&unknown));
    assert_eq!(summary.cost_display(), "unknown");
    let csv = to_csv(&[unknown]);
    assert_eq!(csv.lines().nth(1).unwrap().split(',').nth(5), Some(""));
}

#[test]
fn reported_zero_is_known_and_unpriced_runs_do_not_bias_estimates() {
    let zero = entry(json!(0.0));
    let priced = entry(json!(1.0));
    let unknown = entry(serde_json::Value::Null);
    assert_ne!(CostSummary::from_entries(&[zero]).cost_display(), "unknown");
    let estimate = estimate_cost(
        &[priced, entry(json!(1.0)), entry(json!(1.0)), unknown],
        Some(1),
    )
    .unwrap();
    assert_eq!(estimate.avg_cost_per_turn, 1.0);
    assert_eq!(estimate.sample_size, 3);
}

#[test]
fn a_report_with_any_unreported_session_keeps_its_total_unknown() {
    let summary = CostSummary::from_entries(&[entry(json!(1.0)), entry(serde_json::Value::Null)]);
    assert_eq!(summary.cost_display(), "unknown");
    assert_eq!(summary.total_turns, 2);
}
