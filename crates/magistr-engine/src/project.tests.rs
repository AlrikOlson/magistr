use super::*;

fn sample_context() -> ProjectContext {
    ProjectContext::new(
        vec![
            "abc1234 feat: add widget".into(),
            "def5678 fix: typo".into(),
        ],
        20,
        12,
        vec!["src/lib.rs".into(), "Cargo.toml".into()],
        Some("# Plan\n1. Do stuff".into()),
        Some("Implement widgets".into()),
    )
}

#[test]
fn new_computes_unchecked() {
    let ctx = sample_context();
    assert_eq!(ctx.spec_unchecked, 8);
}

#[test]
fn empty_context() {
    let ctx = ProjectContext::empty();
    assert!(ctx.recent_commits.is_empty());
    assert_eq!(ctx.spec_total, 0);
    assert!(!ctx.has_dirty_files());
    assert!(!ctx.has_active_plan());
    assert!(!ctx.has_active_task());
}

#[test]
fn spec_completion_fraction() {
    let ctx = sample_context();
    assert!((ctx.spec_completion() - 0.6).abs() < f64::EPSILON);
}

#[test]
fn spec_completion_zero_total() {
    let ctx = ProjectContext::empty();
    assert_eq!(ctx.spec_completion(), 0.0);
}

#[test]
fn is_spec_complete_true() {
    let ctx = ProjectContext::new(vec![], 10, 10, vec![], None, None);
    assert!(ctx.is_spec_complete());
}

#[test]
fn is_spec_complete_false() {
    let ctx = sample_context();
    assert!(!ctx.is_spec_complete());
}

#[test]
fn is_spec_complete_empty() {
    assert!(!ProjectContext::empty().is_spec_complete());
}

#[test]
fn has_dirty_files() {
    assert!(sample_context().has_dirty_files());
    assert!(!ProjectContext::empty().has_dirty_files());
}

#[test]
fn has_active_plan() {
    assert!(sample_context().has_active_plan());
    assert!(!ProjectContext::empty().has_active_plan());
}

#[test]
fn has_active_task() {
    assert!(sample_context().has_active_task());
    assert!(!ProjectContext::empty().has_active_task());
}

#[test]
fn latest_commit() {
    let ctx = sample_context();
    assert_eq!(ctx.latest_commit(), Some("abc1234 feat: add widget"));
}

#[test]
fn latest_commit_empty() {
    assert_eq!(ProjectContext::empty().latest_commit(), None);
}

#[test]
fn serialization_roundtrip() {
    let ctx = sample_context();
    let json = serde_json::to_string(&ctx).expect("serialize");
    let parsed: ProjectContext = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(ctx, parsed);
}
