use super::*;
use crate::orchestration_preset::OrchestrationPreset;

#[test]
fn for_task_bug_fix() {
    let rec = RoleRecommendation::for_task("fix the login bug");
    assert!(rec.role_ids.contains(&"coder".to_string()));
    assert!(rec.role_ids.contains(&"tester".to_string()));
    assert!(!rec.description.is_empty());
    // coder + tester is an ambiguous pair, falls back to parallel-tdd
    assert_eq!(rec.suggested_preset, OrchestrationPreset::ParallelTdd);
}

#[test]
fn for_task_new_feature() {
    let rec = RoleRecommendation::for_task("implement new authentication system");
    assert_eq!(rec.role_ids.len(), 3);
    assert_eq!(rec.suggested_preset, OrchestrationPreset::ParallelTdd);
}

#[test]
fn for_task_general() {
    let rec = RoleRecommendation::for_task("do something");
    assert_eq!(rec.role_ids, vec!["coder"]);
    assert_eq!(rec.suggested_preset, OrchestrationPreset::ParallelTdd);
}

#[test]
fn serde_roundtrip() {
    let rec = RoleRecommendation::for_task("fix a bug");
    let json = serde_json::to_string(&rec).unwrap();
    let back: RoleRecommendation = serde_json::from_str(&json).unwrap();
    assert_eq!(back.role_ids, rec.role_ids);
    assert_eq!(back.description, rec.description);
    assert_eq!(back.suggested_preset, rec.suggested_preset);
}
