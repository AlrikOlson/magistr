use std::path::PathBuf;

use super::*;

#[test]
fn partition_strategy_display() {
    assert_eq!(PartitionStrategy::SplitByFile.to_string(), "split-by-file");
    assert_eq!(
        PartitionStrategy::SplitByPhase.to_string(),
        "split-by-phase"
    );
    assert_eq!(
        PartitionStrategy::SplitByComponent.to_string(),
        "split-by-component"
    );
}

#[test]
fn split_by_file_empty_files() {
    let tasks = partition("do stuff", &[], &[], &PartitionStrategy::SplitByFile);
    assert_eq!(tasks.len(), 1);
    assert_eq!(tasks[0].task_id, "file-all");
    assert!(tasks[0].scope.is_empty());
}

#[test]
fn split_by_file_groups_into_chunks() {
    let files: Vec<PathBuf> = (0..12)
        .map(|i| PathBuf::from(format!("file{i}.rs")))
        .collect();
    let tasks = partition(
        "implement feature",
        &files,
        &[],
        &PartitionStrategy::SplitByFile,
    );

    // 12 files / 5 per worker = 3 chunks (5, 5, 2)
    assert_eq!(tasks.len(), 3);
    assert_eq!(tasks[0].scope.len(), 5);
    assert_eq!(tasks[1].scope.len(), 5);
    assert_eq!(tasks[2].scope.len(), 2);

    for (i, task) in tasks.iter().enumerate() {
        assert_eq!(task.task_id, format!("file-{i}"));
        assert_eq!(task.strategy, PartitionStrategy::SplitByFile);
    }
}

#[test]
fn split_by_phase_produces_three_tasks() {
    let tasks = partition(
        "build feature X",
        &[],
        &[],
        &PartitionStrategy::SplitByPhase,
    );
    assert_eq!(tasks.len(), 3);

    assert_eq!(tasks[0].task_id, "phase-research");
    assert!(tasks[0].prompt.contains("RESEARCH ONLY"));

    assert_eq!(tasks[1].task_id, "phase-implement");
    assert!(tasks[1].prompt.contains("IMPLEMENT"));

    assert_eq!(tasks[2].task_id, "phase-test");
    assert!(tasks[2].prompt.contains("WRITE TESTS"));
}

#[test]
fn split_by_component_empty_components() {
    let tasks = partition(
        "update code",
        &[],
        &[],
        &PartitionStrategy::SplitByComponent,
    );
    assert_eq!(tasks.len(), 1);
    assert_eq!(tasks[0].task_id, "component-all");
}

#[test]
fn split_by_component_creates_per_component_tasks() {
    let components = vec!["engine".into(), "tui".into(), "agent".into()];
    let tasks = partition(
        "refactor module",
        &[],
        &components,
        &PartitionStrategy::SplitByComponent,
    );
    assert_eq!(tasks.len(), 3);

    assert_eq!(tasks[0].task_id, "component-0");
    assert!(tasks[0].prompt.contains("engine"));

    assert_eq!(tasks[1].task_id, "component-1");
    assert!(tasks[1].prompt.contains("tui"));

    assert_eq!(tasks[2].task_id, "component-2");
    assert!(tasks[2].prompt.contains("agent"));
}

// ─── auto_select_strategy ────────────────────────────────────────────────

#[test]
fn auto_select_prefers_component_when_multiple_components() {
    let components = vec!["engine".into(), "tui".into()];
    let strategy = auto_select_strategy("build feature", &[], &components);
    assert_eq!(strategy, PartitionStrategy::SplitByComponent);
}

#[test]
fn auto_select_prefers_file_when_many_files() {
    let files: Vec<PathBuf> = (0..8).map(|i| PathBuf::from(format!("f{i}.rs"))).collect();
    let strategy = auto_select_strategy("fix bugs", &files, &[]);
    assert_eq!(strategy, PartitionStrategy::SplitByFile);
}

#[test]
fn auto_select_prefers_phase_for_research_keywords() {
    let strategy = auto_select_strategy("research best practices for auth", &[], &[]);
    assert_eq!(strategy, PartitionStrategy::SplitByPhase);
}

#[test]
fn auto_select_prefers_phase_for_implement_keywords() {
    let strategy = auto_select_strategy("implement a new caching layer", &[], &[]);
    assert_eq!(strategy, PartitionStrategy::SplitByPhase);
}

#[test]
fn auto_select_prefers_file_for_moderate_files() {
    let files = vec![PathBuf::from("a.rs"), PathBuf::from("b.rs")];
    let strategy = auto_select_strategy("fix typos", &files, &[]);
    assert_eq!(strategy, PartitionStrategy::SplitByFile);
}

#[test]
fn auto_select_defaults_to_phase() {
    let strategy = auto_select_strategy("do something", &[], &[]);
    assert_eq!(strategy, PartitionStrategy::SplitByPhase);
}

#[test]
fn auto_select_component_beats_files() {
    let files: Vec<PathBuf> = (0..10).map(|i| PathBuf::from(format!("f{i}.rs"))).collect();
    let components = vec!["mod_a".into(), "mod_b".into()];
    let strategy = auto_select_strategy("refactor", &files, &components);
    assert_eq!(strategy, PartitionStrategy::SplitByComponent);
}

// ─── auto_partition ──────────────────────────────────────────────────────

#[test]
fn auto_partition_uses_selected_strategy() {
    let components = vec!["core".into(), "gui".into()];
    let tasks = auto_partition("refactor modules", &[], &components);
    assert_eq!(tasks.len(), 2);
    assert_eq!(tasks[0].strategy, PartitionStrategy::SplitByComponent);
}
