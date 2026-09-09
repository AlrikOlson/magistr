use super::*;

#[test]
fn bug_fix_recommends_coder_and_tester() {
    let roles = recommend_roles_for_task("fix the login bug");
    assert!(roles.contains(&"coder"));
    assert!(roles.contains(&"tester"));
}

#[test]
fn new_feature_recommends_full_pipeline() {
    let roles = recommend_roles_for_task("implement new authentication system");
    assert!(roles.contains(&"researcher"));
    assert!(roles.contains(&"coder"));
    assert!(roles.contains(&"tester"));
}

#[test]
fn review_task_recommends_reviewer() {
    let roles = recommend_roles_for_task("review the pull request changes");
    assert!(roles.contains(&"reviewer"));
}

#[test]
fn docs_task_recommends_documenter() {
    let roles = recommend_roles_for_task("update the API documentation");
    assert!(roles.contains(&"documenter"));
}

#[test]
fn research_task_recommends_researcher() {
    let roles = recommend_roles_for_task("investigate performance regression");
    assert!(roles.contains(&"researcher"));
}

#[test]
fn refactor_recommends_coder_reviewer_tester() {
    let roles = recommend_roles_for_task("refactor the database module");
    assert!(roles.contains(&"coder"));
    assert!(roles.contains(&"reviewer"));
    assert!(roles.contains(&"tester"));
}

#[test]
fn generic_task_defaults_to_coder() {
    let roles = recommend_roles_for_task("do something");
    assert!(roles.contains(&"coder"));
}

#[test]
fn recommendation_description_is_non_empty() {
    let (roles, desc) = recommend_roles_with_description("fix a bug");
    assert!(!roles.is_empty());
    assert!(!desc.is_empty());
}
