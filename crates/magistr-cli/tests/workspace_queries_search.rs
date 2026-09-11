//! Native search acceptance through the application-owned worker and index APIs.

use std::{
    fs::{self, File, FileTimes},
    path::PathBuf,
    sync::Arc,
    time::{Duration, Instant},
};

use magistr_workspace::{
    config::ResolveOptions,
    contracts::{ErrorKind, Freshness, WorkspaceResponse},
    index::{IndexOptions, WorkspaceIndex, watch::WatchMode},
    query::{
        SearchConfidence, SearchGroup, SearchInclude, SearchInput, SearchMode, WorkspaceQueries,
    },
    worker::{WorkerOptions, WorkerSupervisor},
    workspace_policy::WorkspacePolicy,
};

struct Fixture {
    directory: tempfile::TempDir,
    root: PathBuf,
}

impl Fixture {
    fn new() -> Self {
        let directory = tempfile::tempdir().expect("fixture directory");
        let root = directory.path().join("workspace");
        fs::create_dir_all(&root).expect("workspace");
        Self { directory, root }
    }

    fn write(&self, path: &str, content: &str) {
        let path = self.root.join(path);
        fs::create_dir_all(path.parent().expect("source parent")).expect("source directory");
        fs::write(path, content).expect("source");
    }

    fn policy(&self) -> WorkspacePolicy {
        WorkspacePolicy::resolve(&ResolveOptions::new(&self.root)).expect("workspace policy")
    }

    fn index(&self, worker: &Arc<WorkerSupervisor>) -> WorkspaceIndex {
        WorkspaceIndex::start(
            self.root.clone(),
            self.directory.path().join("state"),
            Arc::clone(worker),
            IndexOptions {
                watch_mode: WatchMode::Off,
                poll_interval: Duration::from_millis(20),
                ..IndexOptions::default()
            },
        )
        .expect("native index")
    }
}

fn worker() -> Arc<WorkerSupervisor> {
    Arc::new(
        WorkerSupervisor::new(
            PathBuf::from(env!("CARGO_BIN_EXE_magistr")),
            WorkerOptions::default(),
        )
        .expect("native worker"),
    )
}

fn rows(response: &WorkspaceResponse<SearchGroup>) -> &[SearchGroup] {
    let WorkspaceResponse::Success { results, .. } = response else {
        panic!("successful native search: {response:?}")
    };
    results.rows()
}

fn error(response: WorkspaceResponse<SearchGroup>) -> ErrorKind {
    let WorkspaceResponse::Failure { error, .. } = response else {
        panic!("typed native refusal: {response:?}")
    };
    error.kind
}

async fn built(index: &WorkspaceIndex, after: u64) {
    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
        let status = index.status();
        if !status.building && status.generation > after {
            return;
        }
        assert!(Instant::now() < deadline, "index did not build: {status:?}");
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
}

#[tokio::test]
async fn cold_lexical_files_filters_context_and_budgets_use_live_sources() {
    let fixture = Fixture::new();
    fixture.write("src/lib.rs", "before\npub fn needle() {}\nafter\n");
    fixture.write("tests/lib.rs", "fn test() { needle(); }\n");
    fixture.write("vendor/lib.rs", "needle();\n");
    fixture.write(".gitignore", "ignored.rs\n");
    fixture.write("ignored.rs", "needle();\n");
    let policy = fixture.policy();
    let worker = worker();
    let queries = WorkspaceQueries::new(&policy, &worker, None).expect("queries");
    let mut input = SearchInput::new("**/*.rs");
    input.mode = SearchMode::Files;
    let files = queries.search(&input).await;
    assert_eq!(rows(&files).len(), 3);
    assert!(rows(&files).iter().all(|row| row.hits.is_empty()));
    assert_eq!(
        worker.status().await.pid,
        None,
        "files do not need a worker"
    );
    input.query = "needle".into();
    input.mode = SearchMode::Lexical;
    let lexical = queries.search(&input).await;
    assert_eq!(rows(&lexical)[0].file, "src/lib.rs");
    assert_eq!(
        rows(&lexical)[0].hits[0].context_before,
        Some(vec!["before".into()])
    );
    assert!(matches!(
        lexical,
        WorkspaceResponse::Success {
            freshness: Freshness::Cold,
            ..
        }
    ));
    input.include = Some(SearchInclude {
        tests: Some(false),
        vendor: Some(false),
    });
    assert_eq!(rows(&queries.search(&input).await).len(), 1);
    input.budget = 1;
    assert_eq!(
        error(queries.search(&input).await),
        ErrorKind::BudgetTooSmall
    );
    input.budget = 4000;
    input.mode = SearchMode::Semantic;
    assert_eq!(
        error(queries.search(&input).await),
        ErrorKind::UnsupportedVersion
    );
    input.mode = SearchMode::References;
    assert_eq!(
        error(queries.search(&input).await),
        ErrorKind::IndexUnavailable
    );
    worker.shutdown().await.expect("reap worker");
}

#[tokio::test]
async fn structural_rust_macros_generics_attributes_and_broken_source_are_native() {
    let fixture = Fixture::new();
    fixture.write("src/lib.rs", "#[derive(Clone)]\nstruct Holder<T> { value: T }\nimpl<T> Holder<T> { fn take(self) -> T { self.value } }\nfn demo() { assert_eq!(1 + 1, 2); }\nfn broken( {\n");
    let policy = fixture.policy();
    let worker = worker();
    let queries = WorkspaceQueries::new(&policy, &worker, None).expect("queries");
    let mut input = SearchInput::new("assert_eq!($$$ARGS)");
    input.mode = SearchMode::Structural;
    input.context = 0;
    let macros = queries.search(&input).await;
    assert_eq!(
        rows(&macros)[0].hits[0].kind.as_deref(),
        Some("macro_invocation")
    );
    input.query = "impl<$$$G> $T { $$$BODY }".into();
    let generic = queries.search(&input).await;
    assert_eq!(rows(&generic)[0].hits[0].kind.as_deref(), Some("impl_item"));
    input.query = "#[derive($$$D)]".into();
    assert_eq!(
        rows(&queries.search(&input).await)[0].hits[0]
            .kind
            .as_deref(),
        Some("attribute_item")
    );
    input.kind = Some(vec!["call".into()]);
    assert!(rows(&queries.search(&input).await).is_empty());
    input.kind = None;
    input.query = "#[derive($$$D)] struct $S { $$$F }".into();
    assert_eq!(
        error(queries.search(&input).await),
        ErrorKind::PatternInvalid
    );
    input.mode = SearchMode::Lexical;
    input.query = "[".into();
    assert_eq!(
        error(queries.search(&input).await),
        ErrorKind::PatternInvalid
    );
    worker.shutdown().await.expect("reap worker");
}

#[tokio::test]
async fn validated_references_keep_confidence_and_recover_same_size_stale_bytes() {
    let fixture = Fixture::new();
    fixture.write("src/needle.ts", "export function needle() {}\n");
    fixture.write(
        "src/app.ts",
        "import { needle } from './needle';\nneedle();\n",
    );
    fixture.write("notes.txt", "needle mentioned\n");
    let policy = fixture.policy();
    let worker = worker();
    let mut index = fixture.index(&worker);
    built(&index, 0).await;
    let queries = WorkspaceQueries::new(&policy, &worker, Some(&index)).expect("queries");
    let mut input = SearchInput::new("needle");
    input.mode = SearchMode::References;
    input.context = 0;
    let response = queries.search(&input).await;
    let groups = rows(&response);
    assert_eq!(groups[0].file, "src/needle.ts");
    assert_eq!(groups[0].hits[0].confidence, Some(SearchConfidence::High));
    assert_eq!(
        groups[0].hits[0].provenance.as_deref(),
        Some("tier1-symbols")
    );
    let application = groups
        .iter()
        .find(|group| group.file == "src/app.ts")
        .expect("import-linked file");
    assert_eq!(
        application.hits[0].confidence,
        Some(SearchConfidence::Medium)
    );
    let note = groups
        .iter()
        .find(|group| group.file == "notes.txt")
        .expect("bare occurrence");
    assert_eq!(note.hits[0].confidence, Some(SearchConfidence::Low));
    let path = fixture.root.join("src/needle.ts");
    let old_time = fs::metadata(&path)
        .expect("metadata")
        .modified()
        .expect("mtime");
    let generation = index.status().generation;
    fixture.write("src/needle.ts", "export function noodle() {}\n");
    File::options()
        .write(true)
        .open(&path)
        .expect("file")
        .set_times(FileTimes::new().set_modified(old_time))
        .expect("restore mtime");
    let stale = queries.search(&input).await;
    match stale {
        WorkspaceResponse::Failure { error, .. } => {
            assert_eq!(error.kind, ErrorKind::IndexUnavailable)
        }
        WorkspaceResponse::Success { results, .. } => assert!(
            results
                .rows()
                .iter()
                .flat_map(|row| &row.hits)
                .all(|hit| hit.kind.as_deref() != Some("definition"))
        ),
    }
    index.request_refresh();
    built(&index, generation).await;
    input.query = "noodle".into();
    assert_eq!(
        rows(&queries.search(&input).await)[0].hits[0]
            .kind
            .as_deref(),
        Some("definition")
    );
    index.shutdown().await.expect("stop index");
    worker.shutdown().await.expect("reap worker");
}

#[tokio::test]
async fn query_budget_cut_is_explicit_and_hits_are_capped_without_losing_counts() {
    let fixture = Fixture::new();
    for number in 0..12 {
        fixture.write(
            &format!("src/file{number:02}.txt"),
            &format!("needle in file {number}\n"),
        );
    }
    fixture.write("src/many.txt", &"needle();\n".repeat(30));
    let policy = fixture.policy();
    let worker = worker();
    let queries = WorkspaceQueries::new(&policy, &worker, None).expect("queries");
    let mut input = SearchInput::new("needle");
    input.mode = SearchMode::Lexical;
    input.context = 0;
    let full = queries.search(&input).await;
    let many = rows(&full)
        .iter()
        .find(|group| group.file == "src/many.txt")
        .expect("many hits");
    assert_eq!(many.total_hits, 30);
    assert_eq!(many.hits.len(), 20);
    input.mode = SearchMode::Files;
    input.query = "**/*.txt".into();
    input.budget = 70;
    let WorkspaceResponse::Success {
        results, budget, ..
    } = queries.search(&input).await
    else {
        panic!("partial groups")
    };
    assert!(budget.used <= 70);
    let cut = results.truncation().expect("manifest");
    assert_eq!(cut.dropped(), 13 - results.rows().len());
    assert!(cut.cursor().starts_with("file:"));
    worker.shutdown().await.expect("reap worker");
}

#[tokio::test]
async fn aggregate_scan_and_hit_bounds_are_explicit_failures() {
    let fixture = Fixture::new();
    let content = "x".repeat(1024 * 1024);
    for number in 0..65 {
        fixture.write(&format!("big/file{number:02}.txt"), &content);
    }
    fixture.write("hits.txt", &"needle\n".repeat(20_001));
    let policy = fixture.policy();
    let worker = worker();
    let queries = WorkspaceQueries::new(&policy, &worker, None).expect("queries");
    let mut input = SearchInput::new("absent");
    input.mode = SearchMode::Lexical;
    input.path = Some("big/**".into());
    assert_eq!(
        error(queries.search(&input).await),
        ErrorKind::BudgetTooSmall
    );
    input.path = Some("hits.txt".into());
    input.query = "needle".into();
    assert_eq!(
        error(queries.search(&input).await),
        ErrorKind::BudgetTooSmall
    );
    worker.shutdown().await.expect("reap worker");
}

#[tokio::test]
async fn newly_added_reference_files_are_explicitly_omitted_until_indexed() {
    let fixture = Fixture::new();
    fixture.write("src/needle.ts", "export function needle() {}\n");
    let policy = fixture.policy();
    let worker = worker();
    let mut index = fixture.index(&worker);
    built(&index, 0).await;
    let generation = index.status().generation;
    fixture.write("src/new.ts", "needle();\n");
    let queries = WorkspaceQueries::new(&policy, &worker, Some(&index)).expect("queries");
    let mut input = SearchInput::new("needle");
    input.mode = SearchMode::References;
    input.context = 0;
    let WorkspaceResponse::Success {
        freshness,
        results,
        notes,
        ..
    } = queries.search(&input).await
    else {
        panic!("existing reference index remains queryable");
    };
    assert_eq!(freshness, Freshness::Warming);
    assert!(
        !results
            .rows()
            .iter()
            .any(|group| group.file == "src/new.ts")
    );
    assert!(
        notes
            .expect("omission note")
            .iter()
            .any(|note| note.contains("absent from the validated index")
                && note.contains("not semantic proof"))
    );
    built(&index, generation).await;
    assert!(
        rows(&queries.search(&input).await)
            .iter()
            .any(|group| group.file == "src/new.ts")
    );
    index.shutdown().await.expect("stop index");
    worker.shutdown().await.expect("reap worker");
}
