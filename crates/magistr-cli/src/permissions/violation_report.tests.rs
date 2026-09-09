use super::*;

use crate::permissions::risk_level::RiskLevel;
use chrono::Utc;

fn entry(reason: &str, allowed: bool, actor: Option<&str>) -> AuditEntry {
    AuditEntry {
        timestamp: Utc::now(),
        tool_name: "Bash".to_string(),
        risk: RiskLevel::High,
        allowed,
        reason: reason.to_string(),
        project: None,
        input_summary: None,
        actor: actor.map(String::from),
    }
}

#[test]
fn an_empty_log_reports_nothing() {
    let report = ViolationReport::from_entries(&[]);
    assert!(report.is_empty());
    assert_eq!(report.total, 0);
}

#[test]
fn only_policy_decided_entries_are_counted() {
    let entries = vec![
        entry("profile default", true, Some("a@b.dev")),
        entry("auto-allow rule: reads", true, Some("a@b.dev")),
        entry("policy rule: no-force-push", false, Some("a@b.dev")),
    ];
    let report = ViolationReport::from_entries(&entries);
    assert_eq!(report.total, 1);
    assert_eq!(report.denied, 1);
    assert_eq!(report.by_rule.get("no-force-push"), Some(&1));
}

#[test]
fn a_warned_but_permitted_entry_still_counts_as_a_violation() {
    let entries = vec![entry("policy rule: note-the-fetches", true, None)];
    let report = ViolationReport::from_entries(&entries);
    assert_eq!(report.total, 1);
    assert_eq!(report.denied, 0);
}

#[test]
fn unattributed_entries_group_rather_than_vanish() {
    let entries = vec![
        entry("policy rule: r", false, None),
        entry("policy rule: r", false, Some("a@b.dev")),
    ];
    let report = ViolationReport::from_entries(&entries);
    assert_eq!(report.by_actor.get(ViolationReport::UNATTRIBUTED), Some(&1));
    assert_eq!(report.by_actor.get("a@b.dev"), Some(&1));
    assert_eq!(report.total, 2);
}

#[test]
fn rules_rank_most_tripped_first_and_ties_break_by_id() {
    let entries = vec![
        entry("policy rule: b-rule", false, None),
        entry("policy rule: a-rule", false, None),
        entry("policy rule: c-rule", false, None),
        entry("policy rule: c-rule", false, None),
    ];
    let report = ViolationReport::from_entries(&entries);
    assert_eq!(
        report.ranked_rules(),
        vec![("c-rule", 2), ("a-rule", 1), ("b-rule", 1)]
    );
}
