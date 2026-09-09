use super::*;
use crate::cost::budget_config::BudgetConfig;
use crate::cost::cost_entry::CostEntry;
use crate::cost::cost_summary::CostSummary;
use chrono::Utc;

#[test]
fn token_breakdown_total() {
    let tb = TokenBreakdown {
        input_tokens: 100,
        output_tokens: 200,
        cache_read_tokens: 50,
        cache_write_tokens: 25,
    };
    assert_eq!(tb.total(), 375);
}

#[test]
fn token_breakdown_accumulate() {
    let mut a = TokenBreakdown {
        input_tokens: 100,
        output_tokens: 200,
        ..Default::default()
    };
    let b = TokenBreakdown {
        input_tokens: 50,
        output_tokens: 75,
        cache_read_tokens: 10,
        cache_write_tokens: 5,
    };
    a.accumulate(&b);
    assert_eq!(a.input_tokens, 150);
    assert_eq!(a.output_tokens, 275);
    assert_eq!(a.cache_read_tokens, 10);
    assert_eq!(a.cache_write_tokens, 5);
}

#[test]
fn budget_config_none_has_no_constraints() {
    let b = BudgetConfig::none();
    assert!(!b.is_set());
    assert_eq!(b.limit, None);
    assert_eq!(b.warn_threshold, None);
}

#[test]
fn budget_config_with_limit() {
    let b = BudgetConfig::with_limit(5.0);
    assert!(b.is_set());
    assert_eq!(b.limit, Some(5.0));
    assert_eq!(b.warn_threshold, None);
}

#[test]
fn budget_config_with_limit_and_warn() {
    let b = BudgetConfig::with_limit_and_warn(5.0, 3.0);
    assert!(b.is_set());
    assert_eq!(b.limit, Some(5.0));
    assert_eq!(b.warn_threshold, Some(3.0));
}

#[test]
fn budget_config_builder_chain() {
    let b = BudgetConfig::none().limit(5.0).warn_threshold(3.0);
    assert!(b.is_set());
    assert_eq!(b.limit, Some(5.0));
    assert_eq!(b.warn_threshold, Some(3.0));
}

#[test]
fn budget_config_builder_limit_only() {
    let b = BudgetConfig::none().limit(10.0);
    assert!(b.is_set());
    assert_eq!(b.limit, Some(10.0));
    assert_eq!(b.warn_threshold, None);
}

#[test]
fn merge_cli_overrides_defaults() {
    let b = BudgetConfig::from_cli_and_defaults(
        Some(10.0), // CLI limit
        Some(8.0),  // CLI warn
        Some(5.0),  // config limit (ignored)
        Some(3.0),  // config warn (ignored)
    );
    assert_eq!(b.limit, Some(10.0));
    assert_eq!(b.warn_threshold, Some(8.0));
}

#[test]
fn merge_falls_back_to_defaults() {
    let b = BudgetConfig::from_cli_and_defaults(
        None,      // no CLI limit
        None,      // no CLI warn
        Some(5.0), // config limit used
        Some(3.0), // config warn used
    );
    assert_eq!(b.limit, Some(5.0));
    assert_eq!(b.warn_threshold, Some(3.0));
}

#[test]
fn merge_partial_cli_partial_defaults() {
    let b = BudgetConfig::from_cli_and_defaults(
        Some(10.0), // CLI limit wins
        None,       // no CLI warn
        Some(5.0),  // config limit ignored (CLI set)
        Some(3.0),  // config warn used
    );
    assert_eq!(b.limit, Some(10.0));
    assert_eq!(b.warn_threshold, Some(3.0));
}

#[test]
fn merge_none_when_nothing_set() {
    let b = BudgetConfig::from_cli_and_defaults(None, None, None, None);
    assert!(!b.is_set());
}

#[test]
fn cost_entry_serialization_round_trip() {
    let entry = CostEntry {
        session_id: "sess-123".into(),
        project: "my-project".into(),
        timestamp: Utc::now(),
        cost_usd: 1.2345,
        tokens: TokenBreakdown {
            input_tokens: 1000,
            output_tokens: 500,
            cache_read_tokens: 200,
            cache_write_tokens: 100,
        },
        turns: 5,
        duration_ms: 30000,
        actor: None,
        model: None,
    };

    let json = serde_json::to_string(&entry).expect("serialize");
    let parsed: CostEntry = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(parsed.session_id, entry.session_id);
    assert_eq!(parsed.cost_usd, entry.cost_usd);
    assert_eq!(parsed.tokens, entry.tokens);
    assert_eq!(parsed.turns, entry.turns);
}

#[test]
fn cost_summary_from_empty() {
    let summary = CostSummary::from_entries(&[]);
    assert_eq!(summary.entry_count, 0);
    assert_eq!(summary.total_cost, 0.0);
    assert_eq!(summary.total_turns, 0);
    assert!(summary.earliest.is_none());
    assert!(summary.latest.is_none());
}

#[test]
fn cost_summary_from_entries() {
    let t1 = Utc::now() - chrono::Duration::hours(2);
    let t2 = Utc::now();
    let entries = vec![
        CostEntry {
            session_id: "a".into(),
            project: "proj".into(),
            timestamp: t1,
            cost_usd: 1.50,
            tokens: TokenBreakdown {
                input_tokens: 100,
                output_tokens: 50,
                ..Default::default()
            },
            turns: 3,
            duration_ms: 10000,
            actor: None,
            model: None,
        },
        CostEntry {
            session_id: "b".into(),
            project: "proj".into(),
            timestamp: t2,
            cost_usd: 2.25,
            tokens: TokenBreakdown {
                input_tokens: 200,
                output_tokens: 100,
                ..Default::default()
            },
            turns: 5,
            duration_ms: 20000,
            actor: None,
            model: None,
        },
    ];

    let summary = CostSummary::from_entries(&entries);
    assert_eq!(summary.entry_count, 2);
    assert!((summary.total_cost - 3.75).abs() < f64::EPSILON);
    assert_eq!(summary.total_turns, 8);
    assert_eq!(summary.total_tokens.input_tokens, 300);
    assert_eq!(summary.total_tokens.output_tokens, 150);
    assert_eq!(summary.earliest, Some(t1));
    assert_eq!(summary.latest, Some(t2));
}

#[test]
fn cost_summary_display_small() {
    let summary = CostSummary {
        total_cost: 0.005,
        total_turns: 1,
        entry_count: 1,
        earliest: None,
        latest: None,
        total_tokens: TokenBreakdown::default(),
    };
    assert_eq!(summary.cost_display(), "$0.0050");
}

#[test]
fn cost_summary_display_large() {
    let summary = CostSummary {
        total_cost: 12.34,
        total_turns: 10,
        entry_count: 5,
        earliest: None,
        latest: None,
        total_tokens: TokenBreakdown::default(),
    };
    assert_eq!(summary.cost_display(), "$12.34");
}
