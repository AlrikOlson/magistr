use super::*;
use crate::cost::types::TokenBreakdown;
use chrono::Utc;
use magistr_engine::cost::BudgetScope;

fn entry(project: &str, actor: Option<&str>) -> CostEntry {
    CostEntry {
        session_id: "s1".into(),
        project: project.into(),
        timestamp: Utc::now(),
        cost_usd: Some(1.0),
        tokens: TokenBreakdown::default(),
        turns: 1,
        duration_ms: 1,
        actor: actor.map(String::from),
        model: None,
    }
}

#[test]
fn org_scope_counts_every_entry_in_the_ledger() {
    let ctx = SpendContext::for_project("magistr");
    assert!(ctx.in_scope(&entry("other", Some("zed")), BudgetScope::Org));
}

#[test]
fn project_scope_matches_the_project_name_case_insensitively() {
    let ctx = SpendContext::for_project("Magistr");
    assert!(ctx.in_scope(&entry("magistr", None), BudgetScope::Project));
    assert!(!ctx.in_scope(&entry("other", None), BudgetScope::Project));
}

#[test]
fn team_scope_without_a_roster_counts_everything() {
    let ctx = SpendContext::for_project("magistr");
    assert!(ctx.in_scope(&entry("magistr", Some("zed")), BudgetScope::Team));
}

#[test]
fn team_scope_with_a_roster_counts_only_its_members() {
    let ctx = SpendContext::for_project("magistr")
        .with_roster(vec!["ada@example.com".into(), "bob@example.com".into()]);
    assert!(ctx.in_scope(
        &entry("magistr", Some("ADA@example.com")),
        BudgetScope::Team
    ));
    assert!(!ctx.in_scope(
        &entry("magistr", Some("zed@example.com")),
        BudgetScope::Team
    ));
}

#[test]
fn unattributed_spend_still_counts_against_the_team() {
    // Older entries have no actor. Excluding them would
    // silently give a team unlimited historical headroom.
    let ctx = SpendContext::for_project("magistr").with_roster(vec!["ada@example.com".into()]);
    assert!(ctx.in_scope(&entry("magistr", None), BudgetScope::Team));
}

#[test]
fn session_scope_counts_nothing_historical() {
    let ctx = SpendContext::for_project("magistr");
    assert!(!ctx.in_scope(&entry("magistr", None), BudgetScope::Session));
}
