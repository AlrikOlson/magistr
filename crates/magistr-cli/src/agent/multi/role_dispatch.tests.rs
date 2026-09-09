use super::*;

#[test]
fn research_keywords_dispatch_to_researcher() {
    assert_eq!(
        classify_subtask_role("phase-research", "Research phase"),
        "researcher"
    );
    assert_eq!(
        classify_subtask_role("investigate-api", "Investigate the API"),
        "researcher"
    );
    assert_eq!(
        classify_subtask_role("analyze-deps", "Analyze dependency graph"),
        "researcher"
    );
}

#[test]
fn implement_keywords_dispatch_to_coder() {
    assert_eq!(
        classify_subtask_role("phase-implement", "Implementation phase"),
        "coder"
    );
    assert_eq!(
        classify_subtask_role("build-feature", "Build the new feature"),
        "coder"
    );
    assert_eq!(
        classify_subtask_role("code-auth", "Code the auth module"),
        "coder"
    );
    assert_eq!(
        classify_subtask_role("fix-bug", "Fix the login bug"),
        "coder"
    );
    assert_eq!(
        classify_subtask_role("refactor-api", "Refactor the API layer"),
        "coder"
    );
}

#[test]
fn review_keywords_dispatch_to_reviewer() {
    assert_eq!(
        classify_subtask_role("review-code", "Review the implementation"),
        "reviewer"
    );
    assert_eq!(
        classify_subtask_role("critique-pr", "Critique the pull request"),
        "reviewer"
    );
    assert_eq!(
        classify_subtask_role("audit-security", "Audit security of auth"),
        "reviewer"
    );
}

#[test]
fn test_keywords_dispatch_to_tester() {
    assert_eq!(
        classify_subtask_role("phase-test", "Testing phase"),
        "tester"
    );
    assert_eq!(
        classify_subtask_role("write-specs", "Write test specs"),
        "tester"
    );
    assert_eq!(
        classify_subtask_role("verify-fix", "Verify the bug fix"),
        "tester"
    );
}

#[test]
fn document_keywords_dispatch_to_documenter() {
    assert_eq!(
        classify_subtask_role("update-docs", "Document the API"),
        "documenter"
    );
    assert_eq!(
        classify_subtask_role("write-readme", "Write documentation"),
        "documenter"
    );
    assert_eq!(
        classify_subtask_role("add-comments", "Add comments to module"),
        "documenter"
    );
}

#[test]
fn unknown_defaults_to_coder() {
    assert_eq!(classify_subtask_role("misc-task", "Do something"), "coder");
}

#[test]
fn case_insensitive_matching() {
    assert_eq!(
        classify_subtask_role("RESEARCH-api", "INVESTIGATE the API"),
        "researcher"
    );
}

#[test]
fn dispatch_with_available_roles() {
    let roles = SpecializedRole::built_in_roles();
    let role = dispatch_to_role("phase-research", "Research the codebase", &roles);
    assert!(role.is_some());
    assert_eq!(role.unwrap().role_id, "researcher");
}

#[test]
fn dispatch_falls_back_when_role_missing() {
    // Only provide coder role
    let roles = vec![SpecializedRole::coder()];
    // Research task should fall back to coder (only available)
    let role = dispatch_to_role("phase-research", "Research phase", &roles);
    assert!(role.is_some());
    assert_eq!(role.unwrap().role_id, "coder");
}

#[test]
fn dispatch_empty_roles_returns_none() {
    let role = dispatch_to_role("phase-research", "Research phase", &[]);
    assert!(role.is_none());
}
