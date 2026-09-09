use super::*;
use crate::cost::types::TokenBreakdown;
use chrono::Utc;

fn entry(session: &str, project: &str, actor: Option<&str>, model: Option<&str>) -> CostEntry {
    CostEntry {
        session_id: session.into(),
        project: project.into(),
        timestamp: Utc::now(),
        cost_usd: 1.0,
        tokens: TokenBreakdown::default(),
        turns: 1,
        duration_ms: 1000,
        actor: actor.map(String::from),
        model: model.map(String::from),
    }
}

#[test]
fn each_dimension_reads_its_own_field() {
    let e = entry(
        "s1",
        "magistr",
        Some("ada@example.com"),
        Some("claude-opus-5"),
    );
    assert_eq!(CostDimension::Session.key_of(&e), "s1");
    assert_eq!(CostDimension::Project.key_of(&e), "magistr");
    assert_eq!(CostDimension::Actor.key_of(&e), "ada@example.com");
    assert_eq!(CostDimension::Model.key_of(&e), "claude-opus-5");
}

#[test]
fn a_missing_attribution_groups_under_unattributed_rather_than_vanishing() {
    let e = entry("s1", "magistr", None, None);
    assert_eq!(CostDimension::Actor.key_of(&e), CostDimension::UNATTRIBUTED);
    assert_eq!(CostDimension::Model.key_of(&e), CostDimension::UNATTRIBUTED);
}

#[test]
fn as_str_names_the_dimension() {
    assert_eq!(CostDimension::Actor.as_str(), "actor");
    assert_eq!(CostDimension::Model.as_str(), "model");
    assert_eq!(CostDimension::Project.as_str(), "project");
    assert_eq!(CostDimension::Session.as_str(), "session");
}
