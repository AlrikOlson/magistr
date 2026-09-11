//! Native read acceptance through admitted source files and the real parser worker.
use std::{
    fs::{self, File, FileTimes},
    path::PathBuf,
    sync::Arc,
    time::{Duration, Instant},
};

use magistr_workspace::{
    config::ResolveOptions,
    contracts::{ErrorKind, WorkspaceResponse},
    index::{IndexOptions, WorkspaceIndex, watch::WatchMode},
    query::{ReadInput, ReadRange, ReadRow, ReadSurround, ReadUnit, WorkspaceQueries},
    worker::{WorkerOptions, WorkerSupervisor},
    workspace_policy::WorkspacePolicy,
};

struct Fixture {
    directory: tempfile::TempDir,
    root: PathBuf,
    policy: WorkspacePolicy,
    worker: Arc<WorkerSupervisor>,
}

impl Fixture {
    fn new() -> Self {
        let directory = tempfile::tempdir().expect("read fixture");
        let root = directory.path().join("workspace");
        fs::create_dir_all(root.join(".git")).expect("repository marker");
        let policy = WorkspacePolicy::resolve(&ResolveOptions::new(&root)).expect("policy");
        let worker = Arc::new(
            WorkerSupervisor::new(
                PathBuf::from(env!("CARGO_BIN_EXE_magistr")),
                WorkerOptions::default(),
            )
            .expect("native worker"),
        );
        Self {
            directory,
            root,
            policy,
            worker,
        }
    }

    fn write(&self, path: &str, source: &str) {
        fs::write(self.root.join(path), source).expect("source fixture");
    }

    fn index(&self) -> WorkspaceIndex {
        WorkspaceIndex::start(
            self.root.clone(),
            self.directory.path().join("state"),
            Arc::clone(&self.worker),
            IndexOptions {
                watch_mode: WatchMode::Off,
                poll_interval: Duration::from_millis(50),
                ..IndexOptions::default()
            },
        )
        .expect("native index")
    }

    fn query<'a>(&'a self, index: Option<&'a WorkspaceIndex>) -> WorkspaceQueries<'a> {
        WorkspaceQueries::new(&self.policy, &self.worker, index).expect("native queries")
    }
}

async fn warm(index: &WorkspaceIndex, after: u64) -> u64 {
    let deadline = Instant::now() + Duration::from_secs(15);
    loop {
        let status = index.status();
        if !status.building && status.generation > after {
            return status.generation;
        }
        assert!(Instant::now() < deadline, "index did not warm: {status:?}");
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
}

fn unit(response: &WorkspaceResponse<ReadRow>) -> &ReadUnit {
    let WorkspaceResponse::Success { results, .. } = response else {
        panic!("expected read success: {response:?}");
    };
    let ReadRow::Unit(unit) = &results.rows()[0] else {
        panic!("unit header");
    };
    unit
}

fn refusal(response: WorkspaceResponse<ReadRow>) -> ErrorKind {
    let WorkspaceResponse::Failure { error, .. } = response else {
        panic!("expected read refusal: {response:?}");
    };
    error.kind
}

fn symbolic(file: &str, address: &str, surround: ReadSurround) -> ReadInput {
    let mut input = ReadInput::new(file);
    input.symbol = Some(address.into());
    input.surround = surround;
    input
}

#[tokio::test]
async fn cold_reads_are_immediate_and_preserve_malformed_source_and_binary_refusals() {
    let fixture = Fixture::new();
    fixture.write("broken.rs", "fn valid() {}\nfn broken( {\n");
    fs::write(fixture.root.join("binary"), [0, 255, 254]).expect("binary fixture");
    let query = fixture.query(None);
    let response = query.read(&ReadInput::new("broken.rs")).await;
    assert_eq!(unit(&response).range, ReadRange { start: 1, end: 2 });
    let WorkspaceResponse::Success { results, .. } = response else {
        unreachable!()
    };
    let ReadRow::Lines(lines) = &results.rows()[1] else {
        panic!("source lines");
    };
    assert!(lines.text.contains("fn broken( {"));
    assert_eq!(
        refusal(
            query
                .read(&symbolic("broken.rs", "valid", ReadSurround::None))
                .await
        ),
        ErrorKind::IndexUnavailable
    );
    assert_eq!(
        refusal(query.read(&ReadInput::new("binary")).await),
        ErrorKind::PatternInvalid
    );
    assert_eq!(
        refusal(query.read(&ReadInput::new("missing.rs")).await),
        ErrorKind::FileNotFound
    );
    assert!(
        fixture.worker.status().await.pid.is_none(),
        "cold reads must not start the parser"
    );
    fixture.worker.shutdown().await.expect("close worker");
}

const RUST: &str = "use std::fmt::Debug;\npub struct Config<T>(pub T);\nimpl<T: Default> Config<T> {\n    /// Loads the default configuration.\n    pub fn load() -> Self { Self(T::default()) }\n    pub fn hello(&self) -> u32 { 1 }\n}\npub trait Greeter { fn hello(&self) -> u32; }\nimpl<T> Greeter for Config<T> {\n    fn hello(&self) -> u32 { 2 }\n}\n";
const TYPESCRIPT: &str = "import { helper } from './helper';\nclass Owner {\n  member() {\n    return helper();\n  }\n}\n";

#[tokio::test]
async fn warm_reads_resolve_generic_and_trait_symbols_and_surrounds() {
    let fixture = Fixture::new();
    fixture.write("config.rs", RUST);
    fixture.write("owner.ts", TYPESCRIPT);
    let mut index = fixture.index();
    warm(&index, 0).await;
    let query = fixture.query(Some(&index));
    for address in ["Config::load", "load", "Config.load"] {
        let response = query
            .read(&symbolic("config.rs", address, ReadSurround::None))
            .await;
        assert_eq!(unit(&response).range, ReadRange { start: 5, end: 5 });
        assert_eq!(
            unit(&response)
                .symbol
                .as_ref()
                .expect("symbol")
                .doc
                .as_deref(),
            Some("Loads the default configuration.")
        );
    }
    let ErrorKind::AmbiguousSymbol { candidates } = refusal(
        query
            .read(&symbolic("config.rs", "Config::hello", ReadSurround::None))
            .await,
    ) else {
        panic!("trait ambiguity");
    };
    assert_eq!(candidates.len(), 2);
    assert!(
        candidates
            .iter()
            .any(|name| name.contains("<Config as Greeter>::hello"))
    );
    let response = query
        .read(&symbolic(
            "owner.ts",
            "Owner.member",
            ReadSurround::Enclosing,
        ))
        .await;
    assert_eq!(unit(&response).range, ReadRange { start: 2, end: 6 });
    assert_eq!(
        unit(&response).enclosing.as_ref().expect("parent").name,
        "Owner"
    );
    let response = query
        .read(&symbolic("owner.ts", "Owner.member", ReadSurround::Imports))
        .await;
    assert_eq!(unit(&response).range, ReadRange { start: 3, end: 5 });
    let WorkspaceResponse::Success { results, .. } = response else {
        unreachable!()
    };
    assert!(
        matches!(&results.rows()[1], ReadRow::Imports(segment) if segment.start == 1 && segment.end == 1)
    );
    index.shutdown().await.expect("stop index");
    fixture.worker.shutdown().await.expect("reap parser");
}

#[tokio::test]
async fn same_size_same_mtime_edits_change_hash_refuse_stale_symbols_and_recover() {
    let fixture = Fixture::new();
    fixture.write("changing.rs", "fn before() {}\n");
    let mut index = fixture.index();
    let generation = warm(&index, 0).await;
    let query = fixture.query(Some(&index));
    let first = query
        .read(&symbolic("changing.rs", "before", ReadSurround::None))
        .await;
    let original_hash = unit(&first).content_hash.clone();
    let path = fixture.root.join("changing.rs");
    let original = fs::metadata(&path).expect("original metadata");
    fixture.write("changing.rs", "fn afterx() {}\n");
    File::options()
        .write(true)
        .open(&path)
        .expect("restore file times")
        .set_times(FileTimes::new().set_modified(original.modified().expect("mtime")))
        .expect("same mtime");
    assert_eq!(
        fs::metadata(&path).expect("edited metadata").len(),
        original.len()
    );
    let current = query.read(&ReadInput::new("changing.rs")).await;
    assert_ne!(unit(&current).content_hash, original_hash);
    assert_eq!(
        refusal(
            query
                .read(&symbolic("changing.rs", "before", ReadSurround::None))
                .await
        ),
        ErrorKind::IndexUnavailable
    );
    index.request_refresh();
    warm(&index, generation).await;
    let recovered = query
        .read(&symbolic("changing.rs", "afterx", ReadSurround::None))
        .await;
    assert_eq!(unit(&recovered).content_hash, unit(&current).content_hash);
    assert_eq!(unit(&recovered).range, ReadRange { start: 1, end: 1 });
    assert_eq!(
        refusal(
            query
                .read(&symbolic("changing.rs", "before", ReadSurround::None))
                .await
        ),
        ErrorKind::SymbolNotFound
    );
    index.shutdown().await.expect("stop index");
    fixture.worker.shutdown().await.expect("reap parser");
}
