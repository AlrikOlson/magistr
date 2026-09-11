//! Shared rig for the native extension and script acceptance suites: a real
//! temp workspace, the actual Magistr parser/sandbox worker executable, the
//! sole journal writer, the workspace key and the reference plugin assets.
// Each test binary compiles this module separately and uses a subset of it;
// the unused remainder in one binary is not dead code in the other.
#![allow(dead_code)]
use std::{
    fs,
    path::{Path, PathBuf},
    sync::{Arc, OnceLock},
};

use magistr_workspace::{
    commands::WorkspaceCommands,
    config::ResolveOptions,
    confirmation::ConfirmationSeal,
    contracts::{ErrorKind, WorkspaceResponse},
    extensions::WorkerExtensionLane,
    jail::WorkspaceKeyStore,
    journal::{JournalDb, JournalDbOptions},
    transaction::FormatterOptions,
    worker::{WorkerOptions, WorkerSupervisor},
    workspace_policy::WorkspacePolicy,
};
use serde_json::{Value, json};

pub(crate) fn reference_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../subsystems/workspace/reference")
}

pub(crate) struct Fixture {
    pub directory: tempfile::TempDir,
    pub root: PathBuf,
    pub policy: WorkspacePolicy,
    pub worker: Arc<WorkerSupervisor>,
    pub journal: JournalDb,
    pub seal: ConfirmationSeal,
}

impl Fixture {
    pub fn new() -> Self {
        let directory = tempfile::tempdir().expect("extension fixture");
        let root = directory.path().join("workspace");
        fs::create_dir_all(root.join(".git")).expect("repository marker");
        fs::create_dir_all(root.join(".workbench")).expect("state dir");
        let key = WorkspaceKeyStore::new(root.join(".workbench"))
            .ensure()
            .expect("workspace key");
        let journal = JournalDb::open(JournalDbOptions::new(root.join(".workbench/journal.db")))
            .expect("sole test journal writer");
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
            journal,
            seal: ConfirmationSeal::new(&key, 1),
        }
    }

    pub fn write(&self, path: &str, content: impl AsRef<[u8]>) {
        let file = self.root.join(path);
        fs::create_dir_all(file.parent().expect("parent")).expect("directory");
        fs::write(file, content).expect("fixture file");
    }

    pub fn read(&self, path: &str) -> Option<String> {
        fs::read_to_string(self.root.join(path)).ok()
    }

    pub fn state_dir(&self) -> PathBuf {
        self.root.join(".workbench")
    }

    pub fn install(&self, records: Vec<Value>) {
        self.write(
            ".workbench/plugins.json",
            json!({ "plugins": records }).to_string(),
        );
    }

    /// Copy a bundled reference plugin (manifest + component) into `plugins/<name>`.
    pub fn copy_plugin(&self, name: &str) -> String {
        let source = reference_dir().join("plugins").join(name);
        let target = format!("plugins/{name}");
        self.write(
            &format!("{target}/plugin.toml"),
            fs::read(source.join("plugin.toml")).expect("bundled manifest"),
        );
        self.write(
            &format!("{target}/guest.wasm"),
            fs::read(source.join("guest.wasm")).expect("bundled component"),
        );
        target
    }

    pub fn lane(&self) -> WorkerExtensionLane {
        WorkerExtensionLane::new(Arc::clone(&self.worker))
    }

    pub fn commands<'a>(&'a mut self, session: &str) -> WorkspaceCommands<'a> {
        WorkspaceCommands::new(
            &self.policy,
            &self.worker,
            &mut self.journal,
            &self.seal,
            None,
            FormatterOptions::default(),
            session,
        )
        .expect("native commands")
    }

    pub async fn shutdown(&self) {
        self.worker.shutdown().await.expect("reap worker child");
    }
}

pub(crate) fn record(
    name: &str,
    source: &str,
    tier: Option<&str>,
    dir: &str,
    grants: Option<&[&str]>,
) -> Value {
    let mut record = json!({ "name": name, "source": source, "dir": dir });
    if let Some(tier) = tier {
        record["tier"] = json!(tier);
    }
    if let Some(grants) = grants {
        record["grants"] = json!(grants);
    }
    record
}

pub(crate) fn ok<R: Clone + std::fmt::Debug>(
    response: WorkspaceResponse<R>,
) -> (Vec<R>, Vec<String>) {
    match response {
        WorkspaceResponse::Success { results, notes, .. } => {
            (results.rows().to_vec(), notes.unwrap_or_default())
        }
        WorkspaceResponse::Failure { error, notes, .. } => {
            panic!("expected success, got {error} {notes:?}")
        }
    }
}

pub(crate) fn refused<R: std::fmt::Debug>(
    response: WorkspaceResponse<R>,
) -> (ErrorKind, String, Vec<String>) {
    match response {
        WorkspaceResponse::Failure { error, notes, .. } => {
            (error.kind, error.message, notes.unwrap_or_default())
        }
        WorkspaceResponse::Success { results, notes, .. } => {
            panic!("expected refusal, got {:?} {notes:?}", results.rows())
        }
    }
}

/// The dependency-free LSP fixture the worker LSP suite also launches.
pub(crate) fn lsp_fixture_binary() -> PathBuf {
    static BINARY: OnceLock<(tempfile::TempDir, PathBuf)> = OnceLock::new();
    BINARY
        .get_or_init(|| {
            let directory = tempfile::tempdir().expect("fixture directory");
            let binary = directory
                .path()
                .join(format!("lsp-fixture{}", std::env::consts::EXE_SUFFIX));
            let source = Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("tests/workspace_worker_lsp_support/fixture.rs");
            let result = std::process::Command::new("rustc")
                .args(["--edition=2024", "--crate-name=workspace_lsp_fixture"])
                .arg(source)
                .arg("-o")
                .arg(&binary)
                .output()
                .expect("compile LSP fixture");
            assert!(
                result.status.success(),
                "fixture compiler: {}",
                String::from_utf8_lossy(&result.stderr)
            );
            (directory, binary)
        })
        .1
        .clone()
}
