use super::*;
use crate::cost::types::TokenBreakdown;
use chrono::{TimeZone, Utc};

fn entry(day: u32, actor: &str, cost: f64) -> CostEntry {
    CostEntry {
        session_id: format!("s{day}-{actor}"),
        project: "magistr".into(),
        timestamp: Utc
            .with_ymd_and_hms(2026, 8, day, 12, 0, 0)
            .single()
            .expect("valid timestamp"),
        cost_usd: cost,
        tokens: TokenBreakdown {
            input_tokens: 10,
            output_tokens: 5,
            ..Default::default()
        },
        turns: 1,
        duration_ms: 1000,
        actor: Some(actor.into()),
        model: Some("claude-opus-5".into()),
    }
}

#[test]
fn an_empty_ledger_charges_nobody() {
    assert!(chargeback(&[], CostDimension::Actor, TimePeriod::Daily).is_empty());
}

#[test]
fn every_period_gets_its_own_rows() {
    let entries = vec![entry(1, "ada", 2.0), entry(2, "ada", 3.0)];
    let rows = chargeback(&entries, CostDimension::Actor, TimePeriod::Daily);
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].period, "2026-08-01");
    assert_eq!(rows[1].period, "2026-08-02");
}

#[test]
fn a_monthly_report_folds_the_days_together() {
    let entries = vec![entry(1, "ada", 2.0), entry(2, "ada", 3.0)];
    let rows = chargeback(&entries, CostDimension::Actor, TimePeriod::Monthly);
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].period, "2026-08");
    assert_eq!(rows[0].cost_usd, 5.0);
    assert_eq!(rows[0].sessions, 2);
}

#[test]
fn share_is_of_the_period_not_the_whole_ledger() {
    // Day 1: ada 3, bob 1. Day 2: bob alone.
    let entries = vec![
        entry(1, "ada", 3.0),
        entry(1, "bob", 1.0),
        entry(2, "bob", 100.0),
    ];
    let rows = chargeback(&entries, CostDimension::Actor, TimePeriod::Daily);
    let day1: Vec<&ChargebackRow> = rows.iter().filter(|r| r.period == "2026-08-01").collect();
    assert_eq!(day1.len(), 2);
    assert!((day1[0].share - 0.75).abs() < 1e-9);
    assert!((day1[1].share - 0.25).abs() < 1e-9);
    let day2: Vec<&ChargebackRow> = rows.iter().filter(|r| r.period == "2026-08-02").collect();
    assert!((day2[0].share - 1.0).abs() < 1e-9);
}

#[test]
fn rows_within_a_period_run_biggest_charge_first() {
    let entries = vec![
        entry(1, "ada", 1.0),
        entry(1, "bob", 5.0),
        entry(1, "cy", 3.0),
    ];
    let rows = chargeback(&entries, CostDimension::Actor, TimePeriod::Daily);
    let keys: Vec<&str> = rows.iter().map(|r| r.key.as_str()).collect();
    assert_eq!(keys, vec!["bob", "cy", "ada"]);
}

#[test]
fn a_row_carries_the_metered_usage_not_just_the_money() {
    let entries = vec![entry(1, "ada", 2.0), entry(1, "ada", 1.0)];
    let rows = chargeback(&entries, CostDimension::Actor, TimePeriod::Daily);
    assert_eq!(rows[0].turns, 2);
    assert_eq!(rows[0].tokens.input_tokens, 20);
    assert_eq!(rows[0].tokens.output_tokens, 10);
    assert_eq!(rows[0].sessions, 2);
}

#[test]
fn the_model_dimension_charges_by_model() {
    let mut a = entry(1, "ada", 2.0);
    a.model = Some("claude-opus-5".into());
    let mut b = entry(1, "bob", 1.0);
    b.model = Some("claude-sonnet-5".into());
    let rows = chargeback(&[a, b], CostDimension::Model, TimePeriod::Monthly);
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].key, "claude-opus-5");
    assert_eq!(rows[1].key, "claude-sonnet-5");
}
