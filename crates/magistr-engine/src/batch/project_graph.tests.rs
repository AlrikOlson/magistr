use super::*;

fn graph(projects: Vec<ProjectDep>) -> ProjectGraph {
    ProjectGraph::new(projects).expect("a well-formed graph")
}

#[test]
fn an_empty_graph_orders_to_nothing() {
    let g = graph(vec![]);
    assert!(g.is_empty());
    assert_eq!(g.len(), 0);
    assert_eq!(g.order().expect("orders"), Vec::<String>::new());
}

#[test]
fn a_duplicate_project_is_refused_at_construction() {
    let err = ProjectGraph::new(vec![ProjectDep::new("api"), ProjectDep::new("api")])
        .expect_err("a duplicate id");
    assert!(err.to_string().contains("declared twice"), "{err}");
}

#[test]
fn a_dependency_comes_before_what_depends_on_it() {
    let g = graph(vec![
        ProjectDep::new("web").after(["api"]),
        ProjectDep::new("api").after(["types"]),
        ProjectDep::new("types"),
    ]);
    assert_eq!(g.order().expect("orders"), vec!["types", "api", "web"]);
}

#[test]
fn independent_projects_are_ordered_by_id_so_the_answer_is_stable() {
    let g = graph(vec![
        ProjectDep::new("zeta"),
        ProjectDep::new("alpha"),
        ProjectDep::new("mid"),
    ]);
    assert_eq!(g.order().expect("orders"), vec!["alpha", "mid", "zeta"]);
}

#[test]
fn a_cycle_is_refused_and_the_refusal_names_its_members() {
    let g = graph(vec![
        ProjectDep::new("a").after(["b"]),
        ProjectDep::new("b").after(["a"]),
        ProjectDep::new("free"),
    ]);
    let err = g.order().expect_err("a cycle cannot be ordered");
    let message = err.to_string();
    assert!(message.contains("a, b"), "{message}");
    assert!(message.contains("cycle"), "{message}");
    assert!(
        !message.contains("free"),
        "the refusal names the cycle, not the whole batch: {message}"
    );
}

#[test]
fn a_self_edge_is_a_cycle() {
    let g = graph(vec![ProjectDep::new("a").after(["a"])]);
    assert!(g.order().is_err(), "a project cannot precede itself");
}

#[test]
fn an_edge_to_a_project_outside_the_batch_is_dropped_not_waited_on() {
    // A batch is a selection. Waiting on a dependency the operator left
    // out would deadlock the run against something it cannot satisfy.
    let g = graph(vec![
        ProjectDep::new("web").after(["api-not-in-this-batch"]),
    ]);
    assert_eq!(g.deps_of("web"), Vec::<&str>::new());
    assert_eq!(g.order().expect("orders"), vec!["web"]);
}

#[test]
fn dependents_are_transitive() {
    let g = graph(vec![
        ProjectDep::new("types"),
        ProjectDep::new("api").after(["types"]),
        ProjectDep::new("web").after(["api"]),
        ProjectDep::new("docs"),
    ]);
    let downstream = g.dependents_of("types");
    assert!(downstream.contains("api"), "direct");
    assert!(downstream.contains("web"), "transitive");
    assert!(!downstream.contains("docs"), "an unrelated branch");
    assert!(!downstream.contains("types"), "never itself");
}

#[test]
fn a_leaf_has_no_dependents() {
    let g = graph(vec![
        ProjectDep::new("api"),
        ProjectDep::new("web").after(["api"]),
    ]);
    assert!(g.dependents_of("web").is_empty());
}

#[test]
fn dependents_of_a_project_in_a_cycle_terminates() {
    // The graph refuses a cycle at ordering time, but `dependents_of` is
    // reachable from a caller holding an unordered graph, and a walk
    // that revisited would not come back.
    let g = graph(vec![
        ProjectDep::new("a").after(["b"]),
        ProjectDep::new("b").after(["a"]),
    ]);
    assert_eq!(g.dependents_of("a"), BTreeSet::from(["b".to_owned()]));
}

#[test]
fn the_graph_answers_what_it_carries() {
    let g = graph(vec![
        ProjectDep::new("api").in_worktree("/repo"),
        ProjectDep::new("web").after(["api"]),
    ]);
    assert_eq!(g.ids(), vec!["api", "web"]);
    assert_eq!(
        g.get("api").expect("known").worktree.as_deref(),
        Some("/repo")
    );
    assert!(g.get("nope").is_none());
    assert_eq!(g.deps_of("web"), vec!["api"]);
    assert_eq!(g.deps_of("nope"), Vec::<&str>::new());
}
