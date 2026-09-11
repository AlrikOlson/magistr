use super::*;

#[test]
fn typed_input_validation_rejects_reserved_and_invalid_requests() {
    let mut input = SearchInput::new("needle");
    input.mode = SearchMode::Semantic;
    assert_eq!(
        validate(&input).expect_err("reserved mode").kind,
        ErrorKind::UnsupportedVersion
    );
    input.mode = SearchMode::Auto;
    input.query.clear();
    assert_eq!(
        validate(&input).expect_err("empty query").kind,
        ErrorKind::PatternInvalid
    );
    input.query = "needle".into();
    input.context = 21;
    assert_eq!(
        validate(&input).expect_err("context bound").kind,
        ErrorKind::PatternInvalid
    );
    input.context = 0;
    input.budget = 0;
    assert_eq!(
        validate(&input).expect_err("budget bound").kind,
        ErrorKind::BudgetTooSmall
    );
}

#[test]
fn context_is_attached_with_retained_blank_line_semantics() {
    let mut group = SearchGroup {
        file: "a.ts".into(),
        junk: false,
        total_hits: 1,
        hits: vec![SearchHit::new(2, 1, "needle();".into())],
    };
    attach_context(&mut group, "before\nneedle();\nafter\n", 2);
    assert_eq!(group.hits[0].context_before, Some(vec!["before".into()]));
    assert_eq!(
        group.hits[0].context_after,
        Some(vec!["after".into(), "".into()])
    );
}

#[tokio::test]
async fn files_and_empty_scopes_do_not_start_the_native_worker() {
    use crate::{
        config::ResolveOptions,
        worker::{WorkerOptions, WorkerSupervisor},
        workspace_policy::WorkspacePolicy,
    };
    let directory = tempfile::tempdir().expect("workspace");
    std::fs::create_dir(directory.path().join("src")).expect("src");
    std::fs::write(directory.path().join("src/lib.rs"), "pub fn needle() {}\n").expect("source");
    let policy = WorkspacePolicy::resolve(&ResolveOptions::new(directory.path())).expect("policy");
    let worker = WorkerSupervisor::new(
        directory.path().join("missing-worker"),
        WorkerOptions::default(),
    )
    .expect("worker");
    let queries = WorkspaceQueries::new(&policy, &worker, None).expect("queries");
    let mut input = SearchInput::new("**/*.rs");
    input.mode = SearchMode::Files;
    let WorkspaceResponse::Success {
        results, freshness, ..
    } = run(&queries, &input).await
    else {
        panic!("files should succeed cold")
    };
    assert_eq!(freshness, Freshness::Cold);
    assert_eq!(results.rows()[0].file, "src/lib.rs");
    input.mode = SearchMode::Lexical;
    input.path = Some("missing/**".into());
    assert!(matches!(
        run(&queries, &input).await,
        WorkspaceResponse::Success { .. }
    ));
    assert_eq!(worker.status().await.pid, None);
    input.mode = SearchMode::References;
    assert!(matches!(
        run(&queries, &input).await,
        WorkspaceResponse::Failure {
            error: WorkspaceError {
                kind: ErrorKind::IndexUnavailable,
                ..
            },
            ..
        }
    ));
}
