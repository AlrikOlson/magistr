use super::*;
use crate::cost::CostEntry;
use crate::cost::types::TokenBreakdown;
use crate::team::TeamConfig;
use crate::team::directory::DirectoryConfig;
use chrono::Utc;
use magistr_engine::cost::{BudgetPeriod, BudgetRule, BudgetScope};

fn roster_with(dir: &std::path::Path, rules: Vec<BudgetRule>) {
    let mut config = TeamConfig {
        version: 1,
        default_role: crate::team::TeamRole::Developer,
        identity_provider: None,
        budgets: Vec::new(),
        policy: Default::default(),
        members: Vec::new(),
        directory: DirectoryConfig::default(),
    };
    config.budgets = rules;
    config.save(dir).expect("save roster");
}

fn ledger_with(path: &std::path::Path, entries: &[CostEntry]) {
    std::fs::write(path, serde_json::to_string(entries).expect("serialize")).expect("write ledger");
}

fn entry(project: &str, cost: f64) -> CostEntry {
    CostEntry {
        session_id: "past".into(),
        project: project.into(),
        timestamp: Utc::now(),
        cost_usd: cost,
        tokens: TokenBreakdown::default(),
        turns: 1,
        duration_ms: 1,
        actor: None,
        model: None,
    }
}

#[test]
fn a_project_with_no_roster_runs_on_its_local_budget_alone() {
    let dir = tempfile::tempdir().expect("tempdir");
    let ledger = dir.path().join("costs.json");
    let local = BudgetConfig::with_limit(7.0);
    let resolved = governed_budget(dir.path(), &ledger, &local, Utc::now());
    assert_eq!(resolved.effective.limit, Some(7.0));
    assert!(!resolved.is_governed());
}

#[test]
fn a_roster_cap_binds_a_larger_local_flag() {
    let dir = tempfile::tempdir().expect("tempdir");
    let ledger = dir.path().join("costs.json");
    roster_with(
        dir.path(),
        vec![BudgetRule::new(BudgetScope::Session, BudgetPeriod::Session).limit(2.0)],
    );
    let resolved = governed_budget(
        dir.path(),
        &ledger,
        &BudgetConfig::with_limit(500.0),
        Utc::now(),
    );
    assert_eq!(resolved.effective.limit, Some(2.0));
    assert_eq!(resolved.binding, Some(BudgetScope::Session));
    assert!(resolved.is_governed());
}

#[test]
fn a_smaller_local_flag_still_wins() {
    let dir = tempfile::tempdir().expect("tempdir");
    let ledger = dir.path().join("costs.json");
    roster_with(
        dir.path(),
        vec![BudgetRule::new(BudgetScope::Session, BudgetPeriod::Session).limit(20.0)],
    );
    let resolved = governed_budget(
        dir.path(),
        &ledger,
        &BudgetConfig::with_limit(1.0),
        Utc::now(),
    );
    assert_eq!(resolved.effective.limit, Some(1.0));
    assert_eq!(resolved.binding, None);
}

#[test]
fn the_ledger_eats_into_a_monthly_project_cap() {
    let dir = tempfile::tempdir().expect("tempdir");
    let project = dir
        .path()
        .file_name()
        .and_then(|n| n.to_str())
        .expect("a project name")
        .to_string();
    let ledger = dir.path().join("costs.json");
    ledger_with(&ledger, &[entry(&project, 18.0)]);
    roster_with(
        dir.path(),
        vec![BudgetRule::new(BudgetScope::Project, BudgetPeriod::Monthly).limit(20.0)],
    );
    let resolved = governed_budget(dir.path(), &ledger, &BudgetConfig::none(), Utc::now());
    assert_eq!(resolved.effective.limit, Some(2.0));
    assert_eq!(resolved.binding, Some(BudgetScope::Project));
}

#[test]
fn another_projects_spend_does_not_eat_this_projects_cap() {
    let dir = tempfile::tempdir().expect("tempdir");
    let ledger = dir.path().join("costs.json");
    ledger_with(&ledger, &[entry("some-other-project", 18.0)]);
    roster_with(
        dir.path(),
        vec![BudgetRule::new(BudgetScope::Project, BudgetPeriod::Monthly).limit(20.0)],
    );
    let resolved = governed_budget(dir.path(), &ledger, &BudgetConfig::none(), Utc::now());
    assert_eq!(resolved.effective.limit, Some(20.0));
}

#[test]
fn an_exhausted_cap_resolves_to_zero() {
    let dir = tempfile::tempdir().expect("tempdir");
    let ledger = dir.path().join("costs.json");
    ledger_with(&ledger, &[entry("anything", 500.0)]);
    roster_with(
        dir.path(),
        vec![BudgetRule::new(BudgetScope::Org, BudgetPeriod::Monthly).limit(100.0)],
    );
    let resolved = governed_budget(dir.path(), &ledger, &BudgetConfig::none(), Utc::now());
    assert_eq!(resolved.effective.limit, Some(0.0));
    assert!(resolved.is_exhausted());
}

#[test]
fn an_unreadable_ledger_still_leaves_the_caps_themselves_in_force() {
    let dir = tempfile::tempdir().expect("tempdir");
    let ledger = dir.path().join("costs.json");
    std::fs::write(&ledger, "{ not json").expect("write ledger");
    roster_with(
        dir.path(),
        vec![BudgetRule::new(BudgetScope::Session, BudgetPeriod::Session).limit(3.0)],
    );
    let resolved = governed_budget(
        dir.path(),
        &ledger,
        &BudgetConfig::with_limit(999.0),
        Utc::now(),
    );
    assert_eq!(resolved.effective.limit, Some(3.0));
}

#[test]
fn the_local_loop_bounds_pass_through_untouched() {
    let dir = tempfile::tempdir().expect("tempdir");
    let ledger = dir.path().join("costs.json");
    let local = BudgetConfig::none().max_iterations(4);
    let resolved = governed_budget(dir.path(), &ledger, &local, Utc::now());
    assert_eq!(resolved.effective.max_iterations, Some(4));
}
