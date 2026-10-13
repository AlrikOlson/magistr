use super::*;
use crate::cost::cost_entry::CostEntry;
use crate::cost::types::TokenBreakdown;
use chrono::Utc;

fn sample_entry(id: &str, project: &str, cost: f64) -> CostEntry {
    CostEntry {
        session_id: id.into(),
        project: project.into(),
        timestamp: Utc::now(),
        cost_usd: Some(cost),
        tokens: TokenBreakdown::default(),
        turns: 3,
        duration_ms: 15000,
        actor: None,
        model: None,
    }
}

#[test]
fn summarize_computes_totals() {
    let entries = vec![sample_entry("s1", "p", 1.50), sample_entry("s2", "p", 2.50)];

    let summary = summarize(&entries);
    assert_eq!(summary.entry_count, 2);
    assert!((summary.total_cost.unwrap() - 4.0).abs() < f64::EPSILON);
    assert_eq!(summary.total_turns, 6);
}
