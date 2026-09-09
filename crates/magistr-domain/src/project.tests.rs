use alloc::vec;

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

#[test]
fn historical_engine_json_preserves_fields_and_counts() {
    let historical = r##"{"recent_commits":["abc1234 feat: add widget"],"spec_total":4294967295,"spec_checked":12,"spec_unchecked":4294967283,"dirty_files":["src/lib.rs"],"active_plan":"# Plan","active_task":null}"##;
    let context: ProjectContext = serde_json::from_str(historical).expect("historical snapshot");
    assert_eq!(context.spec_total, u32::MAX as usize);
    assert_eq!(context.spec_checked, 12);
    assert_eq!(context.spec_unchecked, u32::MAX as usize - 12);
    assert_eq!(
        serde_json::to_string(&context).expect("serialize historical snapshot"),
        historical
    );
}

#[cfg(target_pointer_width = "64")]
#[test]
fn parser_sized_counts_are_not_narrowed_to_u32() {
    let total = u32::MAX as usize + 2;
    let context = ProjectContext::new(vec![], total, total - 1, vec![], None, None);
    let serialized = serde_json::to_string(&context).expect("serialize large counts");
    let restored: ProjectContext = serde_json::from_str(&serialized).expect("restore large counts");
    assert_eq!(restored.spec_total, total);
    assert_eq!(restored.spec_checked, total - 1);
    assert_eq!(restored.spec_unchecked, 1);
    assert!(!restored.is_spec_complete());
}

#[test]
fn over_counted_observation_retains_checked_count_and_saturates_remaining() {
    let context = ProjectContext::new(vec![], 2, 3, vec![], None, None);
    assert_eq!(context.spec_checked, 3);
    assert_eq!(context.spec_unchecked, 0);
    assert_eq!(context.spec_completion(), 1.5);
    assert!(context.is_spec_complete());
}
