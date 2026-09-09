use super::*;
use crate::cost::cost_entry::CostEntry;
use crate::cost::time_period::TimePeriod;
use crate::cost::types::TokenBreakdown;
use chrono::Utc;

fn entry_at(id: &str, project: &str, cost: f64, ts: chrono::DateTime<Utc>) -> CostEntry {
    CostEntry {
        session_id: id.into(),
        project: project.into(),
        timestamp: ts,
        cost_usd: cost,
        tokens: TokenBreakdown::default(),
        turns: 1,
        duration_ms: 1000,
        actor: None,
        model: None,
    }
}

#[test]
fn summarize_daily_groups_by_date() {
    use chrono::TimeZone;
    let d1 = Utc.with_ymd_and_hms(2026, 3, 10, 9, 0, 0).unwrap();
    let d1b = Utc.with_ymd_and_hms(2026, 3, 10, 15, 0, 0).unwrap();
    let d2 = Utc.with_ymd_and_hms(2026, 3, 11, 10, 0, 0).unwrap();

    let entries = vec![
        entry_at("s1", "p", 1.0, d1),
        entry_at("s2", "p", 2.0, d1b),
        entry_at("s3", "p", 3.0, d2),
    ];

    let buckets = summarize_by_period(&entries, TimePeriod::Daily);
    assert_eq!(buckets.len(), 2);
    assert_eq!(buckets[0].label, "2026-03-10");
    assert!((buckets[0].summary.total_cost - 3.0).abs() < f64::EPSILON);
    assert_eq!(buckets[1].label, "2026-03-11");
    assert!((buckets[1].summary.total_cost - 3.0).abs() < f64::EPSILON);
}

#[test]
fn summarize_monthly_groups_by_month() {
    use chrono::TimeZone;
    let jan = Utc.with_ymd_and_hms(2026, 1, 15, 12, 0, 0).unwrap();
    let feb = Utc.with_ymd_and_hms(2026, 2, 20, 12, 0, 0).unwrap();

    let entries = vec![
        entry_at("s1", "p", 5.0, jan),
        entry_at("s2", "p", 10.0, feb),
    ];

    let buckets = summarize_by_period(&entries, TimePeriod::Monthly);
    assert_eq!(buckets.len(), 2);
    assert_eq!(buckets[0].label, "2026-01");
    assert_eq!(buckets[1].label, "2026-02");
}

#[test]
fn summarize_weekly_uses_iso_weeks() {
    use chrono::TimeZone;
    let w11 = Utc.with_ymd_and_hms(2026, 3, 10, 12, 0, 0).unwrap();
    let w12 = Utc.with_ymd_and_hms(2026, 3, 17, 12, 0, 0).unwrap();

    let entries = vec![entry_at("s1", "p", 2.0, w11), entry_at("s2", "p", 4.0, w12)];

    let buckets = summarize_by_period(&entries, TimePeriod::Weekly);
    assert_eq!(buckets.len(), 2);
    assert!(buckets[0].label.contains("-W"));
    assert!(buckets[1].label.contains("-W"));
}

#[test]
fn summarize_empty_entries() {
    let buckets = summarize_by_period(&[], TimePeriod::Daily);
    assert!(buckets.is_empty());
}
