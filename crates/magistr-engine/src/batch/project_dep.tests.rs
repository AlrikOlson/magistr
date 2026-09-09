use super::*;

#[test]
fn a_bare_project_has_no_deps_and_its_own_checkout() {
    let p = ProjectDep::new("api");
    assert_eq!(p.id, "api");
    assert!(p.deps.is_empty());
    assert_eq!(p.worktree, None);
}

#[test]
fn the_builders_declare_deps_and_a_shared_tree() {
    let p = ProjectDep::new("web")
        .after(["api", "types"])
        .in_worktree("/repo");
    assert_eq!(p.deps, vec!["api".to_owned(), "types".to_owned()]);
    assert_eq!(p.worktree.as_deref(), Some("/repo"));
}

#[test]
fn two_projects_naming_one_tree_share_it() {
    let a = ProjectDep::new("api").in_worktree("/repo");
    let b = ProjectDep::new("web").in_worktree("/repo");
    assert!(a.shares_worktree_with(&b));
    assert!(b.shares_worktree_with(&a));
}

#[test]
fn two_projects_naming_different_trees_do_not() {
    let a = ProjectDep::new("api").in_worktree("/repo-a");
    let b = ProjectDep::new("web").in_worktree("/repo-b");
    assert!(!a.shares_worktree_with(&b));
}

#[test]
fn an_undeclared_tree_shares_with_nobody_including_another_undeclared_one() {
    // Two projects that never named a tree are two separate checkouts,
    // not one anonymous one — otherwise every ordinary batch would
    // serialize itself.
    let a = ProjectDep::new("api");
    let b = ProjectDep::new("web");
    assert!(!a.shares_worktree_with(&b));
    assert!(!a.shares_worktree_with(&ProjectDep::new("api").in_worktree("/repo")));
}

#[test]
fn a_dep_record_round_trips_and_a_minimal_one_parses() {
    let p = ProjectDep::new("web").after(["api"]).in_worktree("/repo");
    let json = serde_json::to_string(&p).expect("serialize");
    assert_eq!(serde_json::from_str::<ProjectDep>(&json).expect("parse"), p);

    let minimal: ProjectDep = serde_json::from_str(r#"{"id": "api"}"#).expect("minimal");
    assert_eq!(minimal, ProjectDep::new("api"));
}
