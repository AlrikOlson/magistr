use super::*;
use chrono::TimeZone;

fn at(y: i32, m: u32, d: u32, h: u32, min: u32) -> DateTime<Utc> {
    Utc.with_ymd_and_hms(y, m, d, h, min, 0)
        .single()
        .expect("valid timestamp")
}

#[test]
fn session_period_has_no_window() {
    assert_eq!(
        BudgetPeriod::Session.window_start(at(2026, 8, 29, 4, 30)),
        None
    );
}

#[test]
fn daily_window_starts_at_midnight_utc() {
    let start = BudgetPeriod::Daily
        .window_start(at(2026, 8, 29, 4, 30))
        .expect("daily window");
    assert_eq!(start, at(2026, 8, 29, 0, 0));
}

#[test]
fn weekly_window_starts_on_monday() {
    // 2026-08-29 is a Saturday; its ISO week began Monday 2026-08-24.
    let start = BudgetPeriod::Weekly
        .window_start(at(2026, 8, 29, 4, 30))
        .expect("weekly window");
    assert_eq!(start, at(2026, 8, 24, 0, 0));
}

#[test]
fn weekly_window_on_a_monday_is_that_monday() {
    let start = BudgetPeriod::Weekly
        .window_start(at(2026, 8, 24, 23, 59))
        .expect("weekly window");
    assert_eq!(start, at(2026, 8, 24, 0, 0));
}

#[test]
fn monthly_window_starts_on_the_first() {
    let start = BudgetPeriod::Monthly
        .window_start(at(2026, 8, 29, 4, 30))
        .expect("monthly window");
    assert_eq!(start, at(2026, 8, 1, 0, 0));
}

#[test]
fn serde_round_trips_lowercase() {
    let json = serde_json::to_string(&BudgetPeriod::Monthly).expect("serialize");
    assert_eq!(json, "\"monthly\"");
    let back: BudgetPeriod = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(back, BudgetPeriod::Monthly);
}

#[test]
fn default_is_session() {
    assert_eq!(BudgetPeriod::default(), BudgetPeriod::Session);
}
