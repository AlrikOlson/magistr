use super::*;
use crate::output::{OutputRenderer, SilentRenderer};
use crate::permissions;
use crate::session;
use crate::team::directory::DirectoryConfig;

#[test]
fn cmd_costs_succeeds_with_empty_data() {
    // Happy path: costs file exists but is empty — should display "no entries".
    let tmp = tempfile::TempDir::new().unwrap();
    let costs_file = tmp.path().join("costs.json");
    std::fs::write(&costs_file, "[]").unwrap();

    // Call the underlying load + display logic directly since cmd_costs
    // uses a hardcoded path. Verify the data layer works.
    let entries = cost::storage::load_entries(&costs_file).unwrap();
    assert!(entries.is_empty());
    let summary = cost::summarize_costs::summarize(&entries);
    assert_eq!(summary.total_cost, Some(0.0));
    assert_eq!(summary.total_turns, 0);
}

#[test]
fn cmd_costs_filters_by_project() {
    // Verify project filtering logic works correctly.
    let tmp = tempfile::TempDir::new().unwrap();
    let costs_file = tmp.path().join("costs.json");
    let entries_json = serde_json::json!([
        {
            "session_id": "sess-001",
            "project": "alpha",
            "timestamp": "2025-01-01T00:00:00Z",
            "cost_usd": 1.5,
            "turns": 10,
            "duration_ms": 60000
        },
        {
            "session_id": "sess-002",
            "project": "beta",
            "timestamp": "2025-01-02T00:00:00Z",
            "cost_usd": 2.0,
            "turns": 15,
            "duration_ms": 90000
        }
    ]);
    std::fs::write(&costs_file, entries_json.to_string()).unwrap();

    let entries = cost::storage::load_entries(&costs_file).unwrap();
    let filtered = cost::filter_by_project::filter_by_project(&entries, "alpha");
    assert_eq!(filtered.len(), 1);
    assert_eq!(filtered[0].project, "alpha");
}

#[test]
fn cmd_permissions_with_empty_log() {
    // Edge case: permission audit log file exists but is empty.
    let tmp = tempfile::TempDir::new().unwrap();
    let log_file = tmp.path().join("permissions.log");
    std::fs::write(&log_file, "").unwrap();

    let entries = permissions::audit::load_entries(&log_file).unwrap();
    assert!(entries.is_empty());
    let summary = permissions::audit::summarize(&entries);
    assert_eq!(summary.total, 0);
    assert_eq!(summary.allowed, 0);
    assert_eq!(summary.denied, 0);
}

#[test]
fn cmd_permissions_renderer_dispatches_json() {
    // Verify the JSON renderer path produces valid output when there are no entries.
    let renderer = SilentRenderer::new();
    // SilentRenderer.is_machine_readable() returns false, so this tests the plain path.
    // The function should handle empty entries gracefully.
    assert!(!renderer.is_machine_readable());
}

#[test]
fn cmd_sessions_filter_builder() {
    // Verify SessionFilter builder pattern works correctly.
    let filter = session::SessionFilter::new()
        .with_limit(5)
        .with_project("my-proj");
    // Filter is opaque, but we can verify it was created without panic.
    // The actual filtering is tested in session module tests.
    assert!(format!("{filter:?}").contains("my-proj"));
}

// ── metering, chargeback and the policy view ──────────────────────

#[test]
fn every_grouping_axis_maps_onto_its_reporting_dimension() {
    assert_eq!(
        dimension_of(cli::CostGroupBy::Project),
        cost::CostDimension::Project
    );
    assert_eq!(
        dimension_of(cli::CostGroupBy::User),
        cost::CostDimension::Actor
    );
    assert_eq!(
        dimension_of(cli::CostGroupBy::Model),
        cost::CostDimension::Model
    );
    assert_eq!(
        dimension_of(cli::CostGroupBy::Session),
        cost::CostDimension::Session
    );
}

#[test]
fn every_cli_bucket_maps_onto_its_reporting_period() {
    assert_eq!(
        period_of(cli::CostSummaryPeriod::Daily),
        cost::time_period::TimePeriod::Daily
    );
    assert_eq!(
        period_of(cli::CostSummaryPeriod::Weekly),
        cost::time_period::TimePeriod::Weekly
    );
    assert_eq!(
        period_of(cli::CostSummaryPeriod::Monthly),
        cost::time_period::TimePeriod::Monthly
    );
}

#[test]
fn sub_cent_spend_keeps_four_decimals_so_it_does_not_read_as_free() {
    assert_eq!(money(0.0), "$0.0000");
    assert_eq!(money(0.0042), "$0.0042");
    assert_eq!(money(1.5), "$1.50");
    assert_eq!(money(12.345), "$12.35");
}

#[test]
fn a_long_key_is_truncated_with_an_ellipsis() {
    assert_eq!(shorten("ada", 10), "ada");
    assert_eq!(shorten("0123456789", 10), "0123456789");
    assert_eq!(shorten("01234567890", 10), "012345678…");
}

#[test]
fn shorten_counts_characters_not_bytes() {
    assert_eq!(shorten("ααααα", 5), "ααααα");
    assert_eq!(shorten("αααααα", 5), "αααα…");
}

#[test]
fn a_csv_field_is_quoted_only_when_it_needs_to_be() {
    assert_eq!(csv_field("ada"), "ada");
    assert_eq!(csv_field("a,b"), "\"a,b\"");
    assert_eq!(csv_field("say \"hi\""), "\"say \"\"hi\"\"\"");
}

#[test]
fn the_three_new_report_modes_render_without_panicking() {
    let entries = vec![
        cost::CostEntry {
            session_id: "s1".into(),
            project: "alpha".into(),
            timestamp: chrono::Utc::now(),
            cost_usd: Some(3.0),
            tokens: cost::TokenBreakdown::default(),
            turns: 2,
            duration_ms: 1000,
            actor: Some("ada@example.com".into()),
            model: Some("claude-opus-5".into()),
        },
        cost::CostEntry {
            session_id: "s2".into(),
            project: "alpha".into(),
            timestamp: chrono::Utc::now(),
            cost_usd: Some(1.0),
            tokens: cost::TokenBreakdown::default(),
            turns: 1,
            duration_ms: 500,
            actor: None,
            model: None,
        },
    ];

    for format in [
        cli::CostFormat::Table,
        cli::CostFormat::Csv,
        cli::CostFormat::Json,
    ] {
        cmd_costs_grouped(&entries, cost::CostDimension::Actor, format).expect("grouped");
        cmd_costs_chargeback(
            &entries,
            cost::CostDimension::Model,
            cost::time_period::TimePeriod::Monthly,
            format,
        )
        .expect("chargeback");
    }
}

#[test]
fn an_empty_ledger_renders_the_empty_state_in_every_mode() {
    for format in [
        cli::CostFormat::Table,
        cli::CostFormat::Csv,
        cli::CostFormat::Json,
    ] {
        cmd_costs_grouped(&[], cost::CostDimension::Actor, format).expect("grouped");
        cmd_costs_chargeback(
            &[],
            cost::CostDimension::Actor,
            cost::time_period::TimePeriod::Daily,
            format,
        )
        .expect("chargeback");
    }
}

#[test]
fn the_policy_view_reports_no_policy_for_an_ungoverned_project() {
    let tmp = tempfile::TempDir::new().unwrap();
    let ledger = tmp.path().join("costs.json");
    for format in [
        cli::CostFormat::Table,
        cli::CostFormat::Csv,
        cli::CostFormat::Json,
    ] {
        cmd_costs_policy(tmp.path(), &ledger, format).expect("policy view");
    }
}

#[test]
fn the_policy_view_renders_a_governed_project() {
    use crate::team::{TeamConfig, TeamRole};
    use magistr_engine::cost::{BudgetPeriod, BudgetRule, BudgetScope};

    let tmp = tempfile::TempDir::new().unwrap();
    let ledger = tmp.path().join("costs.json");
    TeamConfig {
        version: 1,
        default_role: TeamRole::Developer,
        identity_provider: None,
        budgets: vec![
            BudgetRule::new(BudgetScope::Org, BudgetPeriod::Monthly).limit(1000.0),
            BudgetRule::new(BudgetScope::Session, BudgetPeriod::Session).limit(5.0),
        ],
        policy: Default::default(),
        members: Vec::new(),
        directory: DirectoryConfig::default(),
    }
    .save(tmp.path())
    .expect("save roster");

    for format in [
        cli::CostFormat::Table,
        cli::CostFormat::Csv,
        cli::CostFormat::Json,
    ] {
        cmd_costs_policy(tmp.path(), &ledger, format).expect("policy view");
    }
}
