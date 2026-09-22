//! Workspace cache acceptance through the real native worker process.

use std::fs::{self, File, FileTimes};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use magistr_workspace::contracts::Freshness;
use magistr_workspace::index::cache::IndexCache;
use magistr_workspace::index::watch::WatchMode;
use magistr_workspace::index::{IndexOptions, IndexStatus, WorkspaceIndex};
use magistr_workspace::worker::{WorkerOptions, WorkerSupervisor};

const WAIT: Duration = Duration::from_secs(15);

struct Fixture {
    _directory: tempfile::TempDir,
    root: PathBuf,
    state: PathBuf,
}

impl Fixture {
    fn new() -> Self {
        let directory = tempfile::tempdir().expect("index fixture directory");
        let root = directory.path().join("workspace");
        fs::create_dir_all(root.join(".git")).expect("workspace repository marker");
        let state = directory.path().join("state");
        Self {
            _directory: directory,
            root,
            state,
        }
    }

    fn write(&self, relative: &str, source: &str) {
        let path = self.root.join(relative);
        fs::create_dir_all(path.parent().expect("fixture parent")).expect("fixture subdirectory");
        fs::write(path, source).expect("fixture source");
    }

    fn start(&self, worker: &Arc<WorkerSupervisor>, options: IndexOptions) -> WorkspaceIndex {
        WorkspaceIndex::start(
            self.root.clone(),
            self.state.clone(),
            Arc::clone(worker),
            options,
        )
        .expect("start workspace index")
    }
}

fn native_worker() -> Arc<WorkerSupervisor> {
    Arc::new(
        WorkerSupervisor::new(
            PathBuf::from(env!("CARGO_BIN_EXE_magistr")),
            WorkerOptions::default(),
        )
        .expect("native worker supervisor"),
    )
}

fn options(mode: WatchMode) -> IndexOptions {
    IndexOptions {
        watch_mode: mode,
        poll_interval: Duration::from_millis(50),
        ..IndexOptions::default()
    }
}

async fn warm(index: &WorkspaceIndex, after: Option<u64>) -> IndexStatus {
    let deadline = Instant::now() + WAIT;
    loop {
        let status = index.status();
        if !status.building
            && !status.refresh_pending
            && status.generation > 0
            && after.is_none_or(|generation| status.generation > generation)
        {
            return status;
        }
        assert!(
            Instant::now() < deadline,
            "index did not warm: generation={}, building={}, freshness={:?}, error={:?}, notes={:?}",
            status.generation,
            status.building,
            status.freshness,
            status.last_error,
            status.notes,
        );
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
}

async fn stop(index: &mut WorkspaceIndex, worker: &WorkerSupervisor) {
    index.shutdown().await.expect("join index coordinator");
    worker.shutdown().await.expect("reap native worker");
}

fn paths(index: &WorkspaceIndex) -> Vec<String> {
    index
        .files(None, 100)
        .expect("indexed files")
        .rows
        .into_iter()
        .map(|file| file.path)
        .collect()
}

#[tokio::test]
async fn refresh_request_is_working_until_a_new_scan_completes() {
    use magistr_workspace::service::{IndexBuildState, IndexBuildTask};
    let fixture = Fixture::new();
    fixture.write("app.py", "value = 1\n");
    let worker = native_worker();
    let mut index = fixture.start(&worker, options(WatchMode::Off));
    let first = warm(&index, None).await;
    index.request_refresh();
    let pending = index.status();
    assert!(pending.refresh_pending);
    assert_eq!(
        IndexBuildTask::derive(&pending).unwrap().state,
        IndexBuildState::Working
    );
    let next = warm(&index, Some(first.generation)).await;
    assert!(!next.refresh_pending);
    assert_eq!(
        IndexBuildTask::derive(&next).unwrap().state,
        IndexBuildState::Completed
    );
    stop(&mut index, &worker).await;
}

#[tokio::test]
async fn native_extraction_fts_pagination_and_reopen_reuse_the_cache() {
    let fixture = Fixture::new();
    let source = "use std::fmt;\npub fn alpha() { /* cachetoken */ }\n#[cfg(test)]\nmod tests {\n    #[test]\n    fn alpha_test() {}\n}\n";
    fixture.write("src/alpha.rs", source);
    fixture.write("src/beta.rs", "pub fn beta() {}\n");
    let worker = native_worker();
    let mut index = fixture.start(&worker, options(WatchMode::Off));
    let first = warm(&index, None).await;
    assert_eq!(first.files, 2);
    assert_eq!(first.parsed + first.reused, 2);

    let query = index.symbols("alpha", 10).expect("symbol prefix query");
    assert_eq!(
        query.freshness,
        Freshness::Warming,
        "watcher-off provenance stays explicit"
    );
    let symbol = query
        .rows
        .iter()
        .find(|row| row.symbol.name == "alpha")
        .expect("worker-extracted alpha symbol");
    assert_eq!(symbol.file.path, "src/alpha.rs");
    assert_eq!(symbol.file.language.as_deref(), Some("rust"));
    assert_eq!(symbol.file.size, source.len() as u64);
    assert!(!symbol.file.content_hash.is_empty());
    assert_eq!(symbol.symbol.start_line, 2);

    let text = index.fts("cachetoken", 10).expect("literal FTS query");
    assert_eq!(text.rows.len(), 1);
    assert_eq!(text.rows[0].path, "src/alpha.rs");
    assert!(
        index
            .fts("\" OR cachetoken", 10)
            .expect("FTS grammar is literal input")
            .rows
            .is_empty()
    );
    let page = index.files(None, 1).expect("first bounded page");
    assert_eq!(page.rows.len(), 1);
    assert!(page.truncated);
    let next = index
        .files(Some(&page.rows[0].path), 1)
        .expect("following page");
    assert_eq!(next.rows.len(), 1);
    assert_ne!(next.rows[0].path, page.rows[0].path);
    assert!(!next.truncated);

    index.request_refresh();
    let unchanged = warm(&index, Some(first.generation)).await;
    assert_eq!(
        unchanged.parsed, 0,
        "unchanged hashes skip worker extraction"
    );
    assert_eq!(unchanged.reused, 2);
    stop(&mut index, &worker).await;
    {
        let stored = IndexCache::open(&fixture.state, "WAL").expect("reopen stopped native cache");
        let file = stored
            .file("src/alpha.rs")
            .expect("stored extraction")
            .expect("stored source");
        assert_eq!(file.content.as_deref(), Some(source));
        assert!(!file.imports.is_empty(), "worker imports are persisted");
        assert!(
            !file.test_ranges.is_empty(),
            "worker test ranges are persisted"
        );
    }

    let reopened_worker = native_worker();
    let mut reopened = fixture.start(&reopened_worker, options(WatchMode::Off));
    let restored = warm(&reopened, Some(unchanged.generation)).await;
    assert_eq!(
        restored.parsed, 0,
        "on-disk cache survives coordinator restart"
    );
    assert_eq!(restored.reused, 2);
    assert_eq!(
        reopened
            .symbols("beta", 10)
            .expect("restored symbol")
            .rows
            .len(),
        1
    );
    assert_eq!(
        reopened_worker.status().await.pid,
        None,
        "reuse needs no parser child"
    );
    stop(&mut reopened, &reopened_worker).await;
}

#[tokio::test]
async fn refresh_applies_edits_deletes_renames_and_preserves_unchanged_rows() {
    let fixture = Fixture::new();
    fixture.write("edit.rs", "pub fn alpha() {}\n");
    fixture.write("delete.rs", "pub fn removed() {}\n");
    fixture.write("rename.rs", "pub fn moved() {}\n");
    fixture.write("stable.rs", "pub fn stable() {}\n");
    let worker = native_worker();
    let mut index = fixture.start(&worker, options(WatchMode::Off));
    let first = warm(&index, None).await;

    fixture.write("edit.rs", "pub fn bravo() {}\n");
    fs::remove_file(fixture.root.join("delete.rs")).expect("delete fixture source");
    fs::rename(
        fixture.root.join("rename.rs"),
        fixture.root.join("moved.rs"),
    )
    .expect("rename fixture source");
    assert!(
        index
            .symbols("alpha", 10)
            .expect("stale edit query")
            .rows
            .is_empty()
    );
    assert!(
        index
            .symbols("removed", 10)
            .expect("deleted query")
            .rows
            .is_empty()
    );
    index.request_refresh();
    let updated = warm(&index, Some(first.generation)).await;
    assert_eq!(paths(&index), ["edit.rs", "moved.rs", "stable.rs"]);
    assert_eq!(
        index
            .symbols("bravo", 10)
            .expect("updated symbol")
            .rows
            .len(),
        1
    );
    let moved = index.symbols("moved", 10).expect("renamed symbol");
    assert_eq!(moved.rows.len(), 1);
    assert_eq!(moved.rows[0].file.path, "moved.rs");
    assert!(updated.reused >= 1, "stable source remains cached");
    stop(&mut index, &worker).await;
}

#[tokio::test]
async fn ignore_changes_remove_cached_files_and_readmit_them() {
    let fixture = Fixture::new();
    fixture.write(".gitignore", "ignored.rs\n");
    fixture.write("ignored.rs", "pub fn ignored() {}\n");
    fixture.write("visible.rs", "pub fn visible() {}\n");
    let worker = native_worker();
    let mut index = fixture.start(&worker, options(WatchMode::Off));
    let first = warm(&index, None).await;
    assert!(
        index
            .symbols("ignored", 10)
            .expect("ignored query")
            .rows
            .is_empty()
    );

    fixture.write(".gitignore", "visible.rs\n");
    assert!(
        index
            .symbols("visible", 10)
            .expect("live ignore rules reject cached candidates")
            .rows
            .is_empty()
    );
    index.request_refresh();
    let second = warm(&index, Some(first.generation)).await;
    assert!(
        index
            .symbols("visible", 10)
            .expect("newly ignored query")
            .rows
            .is_empty()
    );
    assert_eq!(
        index
            .symbols("ignored", 10)
            .expect("readmitted query")
            .rows
            .len(),
        1
    );

    fixture.write(".gitignore", "");
    index.request_refresh();
    warm(&index, Some(second.generation)).await;
    assert_eq!(
        index
            .symbols("visible", 10)
            .expect("visible again")
            .rows
            .len(),
        1
    );
    stop(&mut index, &worker).await;
}

#[tokio::test]
async fn watcher_off_queries_reject_same_size_edits_with_restored_mtime() {
    let fixture = Fixture::new();
    fixture.write("source.rs", "pub fn alpha() { /* oldtoken */ }\n");
    let worker = native_worker();
    let mut index = fixture.start(&worker, options(WatchMode::Off));
    let first = warm(&index, None).await;
    let old_hash = index.files(None, 10).expect("original fingerprint").rows[0]
        .content_hash
        .clone();
    let path = fixture.root.join("source.rs");
    let original = fs::metadata(&path).expect("initial metadata");
    let modified = original.modified().expect("initial modification time");
    fixture.write("source.rs", "pub fn bravo() { /* newtoken */ }\n");
    File::options()
        .write(true)
        .open(&path)
        .expect("fixture file handle")
        .set_times(FileTimes::new().set_modified(modified))
        .expect("restore original modification time");
    let replacement = fs::metadata(&path).expect("replacement metadata");
    assert_eq!(replacement.len(), original.len());
    assert_eq!(replacement.modified().expect("replacement mtime"), modified);

    assert!(
        index
            .symbols("alpha", 10)
            .expect("hash-validated symbols")
            .rows
            .is_empty()
    );
    assert!(
        index
            .fts("oldtoken", 10)
            .expect("hash-validated FTS")
            .rows
            .is_empty()
    );
    let files = index.files(None, 10).expect("hash-validated files");
    assert!(files.rows.iter().all(|file| file.content_hash != old_hash));
    index.request_refresh();
    warm(&index, Some(first.generation)).await;
    assert_eq!(
        index
            .symbols("bravo", 10)
            .expect("fresh replacement")
            .rows
            .len(),
        1
    );
    stop(&mut index, &worker).await;
}

#[cfg(unix)]
#[tokio::test]
async fn watcher_off_queries_validate_mode_changes() {
    use std::os::unix::fs::PermissionsExt;

    let fixture = Fixture::new();
    fixture.write("source.rs", "pub fn alpha() {}\n");
    let worker = native_worker();
    let mut index = fixture.start(&worker, options(WatchMode::Off));
    let first = warm(&index, None).await;
    let path = fixture.root.join("source.rs");
    let old_mode = fs::metadata(&path)
        .expect("mode metadata")
        .permissions()
        .mode();
    let new_mode = old_mode ^ 0o100;
    fs::set_permissions(&path, fs::Permissions::from_mode(new_mode)).expect("change source mode");
    let query = index.symbols("alpha", 10).expect("mode-validated symbols");
    assert!(query.rows.iter().all(|row| row.file.mode == new_mode));
    index.request_refresh();
    warm(&index, Some(first.generation)).await;
    assert_eq!(
        index.files(None, 10).expect("updated mode").rows[0].mode,
        new_mode
    );
    stop(&mut index, &worker).await;
}

#[tokio::test]
async fn native_watcher_refreshes_the_coordinator_without_a_query_or_request() {
    let fixture = Fixture::new();
    fixture.write("source.rs", "pub fn alpha() {}\n");
    let worker = native_worker();
    let mut index = fixture.start(&worker, options(WatchMode::Native));
    let first = warm(&index, None).await;
    fixture.write("source.rs", "pub fn bravo() {}\n");
    warm(&index, Some(first.generation)).await;
    let query = index.symbols("bravo", 10).expect("watcher-updated symbol");
    assert_eq!(query.rows.len(), 1);
    stop(&mut index, &worker).await;
}

#[tokio::test]
async fn cold_queries_and_shutdown_do_not_wait_for_worker_readiness() {
    let fixture = Fixture::new();
    fixture.write("source.rs", "pub fn alpha() {}\n");
    let worker = Arc::new(
        WorkerSupervisor::new(
            PathBuf::from(env!("CARGO_BIN_EXE_magistr")),
            WorkerOptions {
                backoff: vec![Duration::from_secs(30)],
                ..WorkerOptions::default()
            },
        )
        .expect("delayed worker supervisor"),
    );
    let mut index = fixture.start(&worker, options(WatchMode::Off));
    tokio::time::timeout(WAIT, async {
        while worker.pending() == 0 {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .unwrap_or_else(|_| {
        panic!(
            "background extraction was not admitted: {:?}",
            index.status()
        )
    });

    let start = Instant::now();
    for _ in 0..10 {
        let symbols = index.symbols("alpha", 10).expect("cold symbols");
        assert!(symbols.rows.is_empty());
        assert_ne!(symbols.freshness, Freshness::Warm);
        assert!(index.fts("alpha", 10).expect("cold FTS").rows.is_empty());
        assert!(index.files(None, 10).expect("cold files").rows.is_empty());
    }
    assert!(
        start.elapsed() < Duration::from_secs(1),
        "queries waited on worker startup"
    );
    tokio::time::timeout(Duration::from_secs(2), stop(&mut index, &worker))
        .await
        .expect("shutdown cancels pending extraction");
    assert_eq!(worker.pending(), 0);
}

#[cfg(unix)]
#[tokio::test]
async fn owned_worker_crash_is_contained_and_a_later_refresh_restarts_it() {
    let fixture = Fixture::new();
    fixture.write("source.rs", "pub fn alpha() {}\n");
    let worker = native_worker();
    let mut index = fixture.start(&worker, options(WatchMode::Poll));
    let first = warm(&index, None).await;
    let original_pid = worker.status().await.pid.expect("owned parser child pid");
    assert_ne!(original_pid, std::process::id());
    // SAFETY: this PID was returned by this fixture's supervisor for its owned child.
    assert_eq!(unsafe { libc::kill(original_pid as i32, libc::SIGKILL) }, 0);
    fixture.write("source.rs", "pub fn bravo() {}\n");
    assert!(
        index
            .symbols("alpha", 10)
            .expect("query survives worker crash")
            .rows
            .is_empty()
    );
    index.request_refresh();

    let deadline = Instant::now() + WAIT;
    loop {
        let status = index.status();
        if status.freshness == Freshness::Warm && status.generation > first.generation {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "index did not recover: {:?}",
            status.last_error
        );
        if !status.building {
            index.request_refresh();
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    assert_eq!(
        index
            .symbols("bravo", 10)
            .expect("recovered extraction")
            .rows
            .len(),
        1
    );
    let recovered = worker.status().await;
    assert!(recovered.restarts >= 1);
    assert_ne!(recovered.pid, Some(original_pid));
    stop(&mut index, &worker).await;
    assert_eq!(worker.status().await.pid, None);
}

#[tokio::test]
async fn oversized_files_and_workspace_limits_are_bounded_and_reported() {
    let fixture = Fixture::new();
    fixture.write(
        "a_oversized.rs",
        &format!("// excludedtoken {}\n", "x".repeat(1024)),
    );
    for n in 0..8 {
        fixture.write(
            &format!("source{n}.rs"),
            &format!("pub fn symbol{n}() {{}}\n"),
        );
    }
    let worker = native_worker();
    let mut index = fixture.start(
        &worker,
        IndexOptions {
            max_file_bytes: 128,
            max_files: 3,
            ..options(WatchMode::Off)
        },
    );
    let status = warm(&index, None).await;
    assert!(status.files <= 3);
    assert!(status.parsed <= 3);
    let query = index.files(None, 100).expect("bounded inventory");
    assert!(query.rows.len() <= 3);
    assert!(
        query
            .rows
            .iter()
            .filter(|file| file.size > 128)
            .all(|file| file.content.is_none() && file.symbols.is_empty())
    );
    assert!(
        status
            .notes
            .iter()
            .chain(&query.notes)
            .chain(status.last_error.iter())
            .any(|note| note.contains("limit")
                || note.contains("bound")
                || note.contains("incomplete")),
        "the admission limit must be visible: status={status:?}, query={query:?}"
    );
    assert!(
        index
            .fts("excludedtoken", 10)
            .expect("oversized exclusion")
            .rows
            .is_empty()
    );
    stop(&mut index, &worker).await;
}
