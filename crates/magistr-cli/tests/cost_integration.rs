//! Integration tests for cost tracking, budget enforcement, and reporting.
//!
//! Tests the full flow: CostTracker accumulates turns → persists via storage →
//! filters and summarizes → exports to CSV. Exercises cross-module interactions
//! between `cost::CostTracker`, `cost::budget`, `cost::storage`, and `cost::types`.

use chrono::{TimeZone, Utc};
use tempfile::TempDir;

use magistr::cost::CostTracker;
use magistr::cost::append_entry::append_entry;
use magistr::cost::budget::BudgetStatus;
use magistr::cost::budget_config::BudgetConfig;
use magistr::cost::cost_entry::CostEntry;
use magistr::cost::cost_summary::CostSummary;
use magistr::cost::csv_export::to_csv;
use magistr::cost::filter_by_project::filter_by_project;
use magistr::cost::storage::load_entries;
use magistr::cost::summarize_by_period::summarize_by_period;
use magistr::cost::summarize_costs::summarize;
use magistr::cost::time_period::TimePeriod;
use magistr::cost::types::TokenBreakdown;

// ── Helpers ────────────────────────────────────────────────────────

fn make_entry(id: &str, project: &str, cost: f64, tokens: TokenBreakdown) -> CostEntry {
    CostEntry {
        session_id: id.into(),
        project: project.into(),
        timestamp: Utc::now(),
        cost_usd: Some(cost),
        tokens,
        turns: 5,
        duration_ms: 30000,
        actor: None,
        model: None,
    }
}

fn make_tokens(input: u64, output: u64) -> TokenBreakdown {
    TokenBreakdown {
        input_tokens: input,
        output_tokens: output,
        cache_read_tokens: 0,
        cache_write_tokens: 0,
    }
}

// ── CostTracker → Storage round-trip ───────────────────────────────

#[test]
fn tracker_to_storage_round_trip() {
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("costs.json");

    // Simulate a session with CostTracker
    let mut tracker = CostTracker::new(BudgetConfig::with_limit(10.0));
    let tokens = make_tokens(1000, 500);
    tracker.record_turn(1.50, 5000, Some(&tokens));
    tracker.record_turn(2.00, 3000, Some(&tokens));
    tracker.record_turn(0.75, 2000, None);

    // Persist via storage
    let entry = tracker.to_cost_entry("session-001", "my-project");
    append_entry(&path, &entry).unwrap();

    // Load back and verify
    let loaded = load_entries(&path).unwrap();
    assert_eq!(loaded.len(), 1);
    assert_eq!(loaded[0].session_id, "session-001");
    assert_eq!(loaded[0].project, "my-project");
    assert!((loaded[0].cost_usd.unwrap() - 4.25).abs() < f64::EPSILON);
    assert_eq!(loaded[0].turns, 3);
    assert_eq!(loaded[0].duration_ms, 10000);
    assert_eq!(loaded[0].tokens.input_tokens, 2000);
    assert_eq!(loaded[0].tokens.output_tokens, 1000);
}

// ── Budget enforcement through CostTracker ─────────────────────────

#[test]
fn budget_lifecycle_warning_then_exceeded() {
    let mut tracker = CostTracker::new(BudgetConfig::with_limit_and_warn(5.0, 3.0));

    // Turn 1: under warning
    let s1 = tracker.record_turn(2.0, 1000, None);
    assert_eq!(s1, BudgetStatus::Ok);

    // Turn 2: crosses warning threshold
    let s2 = tracker.record_turn(1.5, 1000, None);
    assert!(s2.should_warn());
    assert!(!s2.should_stop());

    // Turn 3: warning already issued, still under limit
    let s3 = tracker.record_turn(0.5, 1000, None);
    assert_eq!(s3, BudgetStatus::Ok);

    // Turn 4: exceeds hard limit
    let s4 = tracker.record_turn(2.0, 1000, None);
    assert!(s4.should_stop());
}

// ── Multi-session storage with filtering ───────────────────────────

#[test]
fn multi_session_filter_and_summarize() {
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("costs.json");

    let e1 = make_entry("s1", "frontend", 2.50, make_tokens(500, 200));
    let e2 = make_entry("s2", "backend-api", 5.00, make_tokens(1000, 400));
    let e3 = make_entry("s3", "frontend-v2", 3.00, make_tokens(700, 300));

    append_entry(&path, &e1).unwrap();
    append_entry(&path, &e2).unwrap();
    append_entry(&path, &e3).unwrap();

    let all = load_entries(&path).unwrap();
    assert_eq!(all.len(), 3);

    // Filter by project
    let frontend = filter_by_project(&all, "frontend");
    assert_eq!(frontend.len(), 2);

    // Summarize filtered
    let summary = summarize(&frontend);
    assert_eq!(summary.entry_count, 2);
    assert!((summary.total_cost.unwrap() - 5.50).abs() < f64::EPSILON);
    assert_eq!(summary.total_turns, 10);
    assert_eq!(summary.total_tokens.input_tokens, 1200);
}

// ── CSV export from persisted data ─────────────────────────────────

#[test]
fn csv_export_from_storage() {
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("costs.json");

    let e1 = make_entry("s1", "project-a", 1.23, make_tokens(100, 50));
    let e2 = make_entry("s2", "project-b", 4.56, make_tokens(200, 100));

    append_entry(&path, &e1).unwrap();
    append_entry(&path, &e2).unwrap();

    let entries = load_entries(&path).unwrap();
    let csv = to_csv(&entries);

    let lines: Vec<&str> = csv.lines().collect();
    assert_eq!(lines.len(), 3); // header + 2 entries
    assert!(lines[0].starts_with("session_id,"));
    assert!(lines[1].starts_with("s1,project-a,"));
    assert!(lines[2].starts_with("s2,project-b,"));
    assert!(lines[1].contains("1.230000"));
}

// ── Time-bucketed summaries with real data ─────────────────────────

#[test]
fn daily_summary_groups_sessions() {
    let d1 = Utc.with_ymd_and_hms(2026, 3, 10, 9, 0, 0).unwrap();
    let d1b = Utc.with_ymd_and_hms(2026, 3, 10, 14, 0, 0).unwrap();
    let d2 = Utc.with_ymd_and_hms(2026, 3, 11, 10, 0, 0).unwrap();

    let entries = vec![
        CostEntry {
            session_id: "s1".into(),
            project: "p".into(),
            timestamp: d1,
            cost_usd: Some(2.0),
            tokens: make_tokens(100, 50),
            turns: 3,
            duration_ms: 5000,
            actor: None,
            model: None,
        },
        CostEntry {
            session_id: "s2".into(),
            project: "p".into(),
            timestamp: d1b,
            cost_usd: Some(3.0),
            tokens: make_tokens(200, 100),
            turns: 4,
            duration_ms: 8000,
            actor: None,
            model: None,
        },
        CostEntry {
            session_id: "s3".into(),
            project: "p".into(),
            timestamp: d2,
            cost_usd: Some(1.5),
            tokens: make_tokens(150, 75),
            turns: 2,
            duration_ms: 3000,
            actor: None,
            model: None,
        },
    ];

    let buckets = summarize_by_period(&entries, TimePeriod::Daily);
    assert_eq!(buckets.len(), 2);
    assert_eq!(buckets[0].label, "2026-03-10");
    assert!((buckets[0].summary.total_cost.unwrap() - 5.0).abs() < f64::EPSILON);
    assert_eq!(buckets[0].summary.total_turns, 7);
    assert_eq!(buckets[0].summary.total_tokens.input_tokens, 300);
    assert_eq!(buckets[1].label, "2026-03-11");
    assert!((buckets[1].summary.total_cost.unwrap() - 1.5).abs() < f64::EPSILON);
}

// ── Budget check with no budget set ────────────────────────────────

#[test]
fn no_budget_never_stops() {
    let mut tracker = CostTracker::new(BudgetConfig::none());
    for _ in 0..100 {
        let status = tracker.record_turn(100.0, 1000, None);
        assert_eq!(status, BudgetStatus::Ok);
    }
    assert!((tracker.total_cost() - 10000.0).abs() < f64::EPSILON);
}

// ── CostSummary from tracker-generated entries ─────────────────────

#[test]
fn summary_from_multiple_tracker_entries() {
    let mut t1 = CostTracker::new(BudgetConfig::none());
    t1.record_turn(1.0, 2000, Some(&make_tokens(100, 50)));
    t1.record_turn(1.5, 3000, Some(&make_tokens(200, 100)));

    let mut t2 = CostTracker::new(BudgetConfig::none());
    t2.record_turn(3.0, 5000, Some(&make_tokens(500, 250)));

    let entries = vec![
        t1.to_cost_entry("s1", "project"),
        t2.to_cost_entry("s2", "project"),
    ];

    let summary = CostSummary::from_entries(&entries);
    assert_eq!(summary.entry_count, 2);
    assert!((summary.total_cost.unwrap() - 5.5).abs() < f64::EPSILON);
    assert_eq!(summary.total_turns, 3); // 2 + 1
    assert_eq!(summary.total_tokens.input_tokens, 800);
    assert_eq!(summary.total_tokens.output_tokens, 400);
}
