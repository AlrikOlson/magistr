use super::*;

// ─── is_research_task classification ───────────────────────────────────────

#[test]
fn classifies_research_tasks_by_id() {
    assert!(is_research_task("research-api", "Look into the API"));
    assert!(is_research_task("phase-research", "Find patterns"));
}

#[test]
fn classifies_research_tasks_by_description() {
    assert!(is_research_task(
        "task-1",
        "Research best practices for auth"
    ));
    assert!(is_research_task(
        "task-2",
        "Investigate the bug in module X"
    ));
    assert!(is_research_task("task-3", "Explore codebase structure"));
    assert!(is_research_task(
        "task-4",
        "Analyze performance bottlenecks"
    ));
}

#[test]
fn non_research_tasks_classified_correctly() {
    assert!(!is_research_task("implement-auth", "Add login endpoint"));
    assert!(!is_research_task("test-suite", "Write integration tests"));
    assert!(!is_research_task("refactor-db", "Clean up database layer"));
}

// ─── is_reviewer_task classification ───────────────────────────────────────

#[test]
fn classifies_reviewer_tasks_by_id() {
    assert!(is_reviewer_task("reviewer", "Check the code"));
    assert!(is_reviewer_task("code-review", "Assess quality"));
}

#[test]
fn classifies_reviewer_tasks_by_description() {
    assert!(is_reviewer_task("task-1", "Review the implementation"));
    assert!(is_reviewer_task("task-2", "Critique the code changes"));
}

#[test]
fn non_reviewer_tasks_classified_correctly() {
    assert!(!is_reviewer_task("implement", "Add new feature"));
    assert!(!is_reviewer_task("test", "Write tests for module"));
}

// ─── is_approved detection ─────────────────────────────────────────────────

#[test]
fn detects_approval() {
    assert!(is_approved("APPROVED: Looks good"));
    assert!(is_approved("approved: minor nits but acceptable"));
    assert!(is_approved("The code is APPROVED."));
}

#[test]
fn detects_non_approval() {
    assert!(!is_approved("NOT APPROVED: needs changes"));
    assert!(!is_approved("REVISIONS NEEDED: fix the error handling"));
    assert!(!is_approved("The code needs more work"));
}

// ─── build_context_prefix ──────────────────────────────────────────────────

#[test]
fn empty_context_produces_empty_prefix() {
    let ctx = SharedContext::new();
    assert!(build_context_prefix(&ctx).is_empty());
}

#[test]
fn context_with_findings_produces_prefix() {
    let ctx = SharedContext::new();
    let agent = super::super::topology::AgentId::new(1);
    ctx.publish(agent, "research_findings", "Found pattern X");
    ctx.publish(agent, "research_findings", "API uses REST");

    let prefix = build_context_prefix(&ctx);
    assert!(prefix.contains("Finding 1"));
    assert!(prefix.contains("Finding 2"));
    assert!(prefix.contains("Found pattern X"));
    assert!(prefix.contains("API uses REST"));
}

// ─── ReviewPairExecutor defaults ───────────────────────────────────────────

#[test]
fn review_pair_default_max_rounds() {
    let executor = ReviewPairExecutor::default();
    assert_eq!(executor.max_rounds, 3);
}

// ─── build_review_task ─────────────────────────────────────────────────────

#[test]
fn builds_review_task_without_template() {
    let task = build_review_task(2, "code changes here", None, std::path::Path::new("/tmp"));
    assert_eq!(task.task_id, "reviewer-round-2");
    assert!(task.prompt.contains("Round 2"));
    assert!(task.prompt.contains("code changes here"));
    assert!(task.prompt.contains("APPROVED"));
}

#[test]
fn builds_review_task_with_template() {
    let template = SubTask {
        task_id: "my-reviewer".into(),
        description: "Custom review".into(),
        prompt: "Check for security issues".into(),
        scope: vec![std::path::PathBuf::from("/src")],
        strategy: super::super::partition::PartitionStrategy::SplitByFile,
        archetype_id: None,
    };
    let task = build_review_task(
        1,
        "diff output",
        Some(&template),
        std::path::Path::new("/tmp"),
    );
    assert_eq!(task.task_id, "my-reviewer-round-1");
    assert!(task.prompt.contains("Check for security issues"));
    assert!(task.prompt.contains("diff output"));
}

// ─── DrainState ────────────────────────────────────────────────────────────

#[test]
fn drain_state_starts_empty() {
    let state = DrainState::new();
    assert!(state.results.is_empty());
    assert!(state.retry_counts.is_empty());
}
