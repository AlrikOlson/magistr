//! Admitted language packs through the real owner (audit-260912-24): the
//! Gleam reference pack is registered into the first worker incarnation at
//! open, selected by the host for the index and for parse verification, and
//! restored into the replacement worker after a forced kill; a pack that
//! fails to register is a status row and a breach, and a workspace without
//! packs never spawns a worker at open.
use std::{
    fs,
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

use magistr_workspace::{
    commands::WriteInput,
    contracts::{ErrorKind, WorkspaceResponse},
    extensions::PluginHealth,
    index::IndexStatus,
    query::{SearchGroup, SearchInput, SearchMode},
    service::{WorkspaceOpenOptions, WorkspaceService},
};
use serde_json::json;

const WAIT: Duration = Duration::from_secs(30);
const GLEAM_FIXTURE: &str = "import gleam/io\n\npub fn main() {\n  io.println(greet(\"world\"))\n}\n\nfn greet(name: String) -> String {\n  \"hello\"\n}\n\npub type Season {\n  Spring\n  Summer\n}\n\npub const answer = 42\n";
const BROKEN_GLEAM: &str = "pub fn broken( {\n";

fn reference_pack() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../magistr-workspace/fixtures/plugins/lang-pack-gleam")
}

/// A workspace with the Gleam pack installed (`component` selects the carrier
/// bytes: the real guest or corrupt bytes) and one Gleam source file.
fn workspace(component: Option<&[u8]>) -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("workspace");
    let root = dir.path();
    fs::create_dir_all(root.join(".git")).expect("repository marker");
    fs::create_dir_all(root.join(".magistr/workspace")).expect("state dir");
    fs::create_dir_all(root.join("src")).expect("source dir");
    fs::write(root.join("src/main.gleam"), GLEAM_FIXTURE).expect("gleam source");
    if let Some(component) = component {
        fs::create_dir_all(root.join("plugins/lang-pack-gleam")).expect("plugin dir");
        fs::copy(
            reference_pack().join("plugin.toml"),
            root.join("plugins/lang-pack-gleam/plugin.toml"),
        )
        .expect("manifest");
        fs::write(root.join("plugins/lang-pack-gleam/guest.wasm"), component).expect("carrier");
        fs::write(
            root.join(".magistr/workspace/plugins.json"),
            json!({ "plugins": [{
                "name": "magistr.lang-pack-gleam",
                "source": "registry",
                "tier": "verified",
                "dir": "plugins/lang-pack-gleam",
            }] })
            .to_string(),
        )
        .expect("inventory");
    }
    dir
}

async fn open(root: &Path) -> WorkspaceService {
    let mut flags = serde_json::Map::new();
    flags.insert("watcher.mode".into(), json!("off"));
    WorkspaceService::open(
        WorkspaceOpenOptions::new(root, env!("CARGO_BIN_EXE_magistr"))
            .with_network_override("0")
            .with_session_flags(flags),
    )
    .await
    .expect("admitted workspace")
}

async fn built(service: &WorkspaceService, after: u64) -> IndexStatus {
    let deadline = Instant::now() + WAIT;
    loop {
        let status = service.index_status();
        if !status.building && !status.refresh_pending && status.generation > after {
            return status;
        }
        assert!(Instant::now() < deadline, "index did not build: {status:?}");
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}

async fn references(service: &WorkspaceService, symbol: &str) -> WorkspaceResponse<SearchGroup> {
    let mut input = SearchInput::new(symbol);
    input.mode = SearchMode::References;
    service
        .search(&input, tokio_util::sync::CancellationToken::new())
        .await
}

async fn defining_files(service: &WorkspaceService, symbol: &str) -> Vec<String> {
    match references(service, symbol).await {
        WorkspaceResponse::Success { results, .. } => results
            .rows()
            .iter()
            .map(|group| group.file.clone())
            .collect(),
        WorkspaceResponse::Failure { error, notes, .. } => {
            panic!("references search failed: {error} {notes:?}")
        }
    }
}

#[cfg(unix)]
fn kill(pid: u32) {
    // SAFETY: the pid is this test's own worker child, read from the owner a
    // moment ago; SIGKILL is the forced replacement the acceptance names.
    unsafe {
        libc::kill(pid as i32, libc::SIGKILL);
    }
}

/// The same forced end where there are no signals.
#[cfg(windows)]
fn kill(pid: u32) {
    let status = std::process::Command::new("taskkill")
        .args(["/PID", &pid.to_string(), "/F"])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .expect("taskkill runs");
    assert!(status.success(), "taskkill ends worker {pid}");
}

#[tokio::test]
async fn the_gleam_pack_is_registered_selected_and_restored_after_a_forced_worker_kill() {
    let dir = workspace(Some(
        &fs::read(reference_pack().join("guest.wasm")).expect("guest"),
    ));
    let service = open(dir.path()).await;
    let session = service.new_session().expect("session");

    // Registered into the first incarnation at open; the owner retains it.
    let statuses = service.lang_pack_statuses();
    assert_eq!(statuses.len(), 1, "{statuses:?}");
    assert!(statuses[0].ok, "{statuses:?}");
    assert_eq!(statuses[0].language.as_deref(), Some("gleam"));
    assert_eq!(statuses[0].lane.as_deref(), Some("wasm"));
    assert_eq!(
        statuses[0].extensions.as_deref(),
        Some(&["gleam".to_owned()][..])
    );
    assert_eq!(
        service.languages().language("src/main.gleam").as_deref(),
        Some("gleam"),
        "host-side selection consults the retained registration"
    );
    assert_eq!(
        service.languages().language("src/lib.rs").as_deref(),
        Some("rust"),
        "bundled languages are never shadowed"
    );
    let WorkspaceResponse::Success { notes, .. } = service.status() else {
        panic!("status");
    };
    assert!(
        notes.as_deref().unwrap_or_default().iter().any(|note| note
            == "lang pack magistr.lang-pack-gleam: gleam registered on the wasm lane (extensions: gleam)"),
        "{notes:?}"
    );

    // The index extracts the Gleam file through the worker: Tier-1 symbols.
    let first_build = built(&service, 0).await;
    // The plugin's manifest and carrier are files too; only the Gleam source
    // has a grammar.
    assert_eq!(first_build.files, 3, "{first_build:?}");
    assert_eq!(
        first_build.parsed + first_build.reused,
        first_build.files,
        "{first_build:?}"
    );
    assert_eq!(
        defining_files(&service, "greet").await,
        vec!["src/main.gleam"]
    );

    // Parse verification reaches the registered grammar.
    let refused = service
        .write(&WriteInput::new("src/broken.gleam", BROKEN_GLEAM), &session)
        .await;
    let WorkspaceResponse::Failure { error, .. } = refused else {
        panic!("broken gleam must be refused: {refused:?}");
    };
    assert!(
        matches!(error.kind, ErrorKind::ParseRejected { .. }),
        "{error}"
    );

    // Forced replacement: kill the admitted incarnation.
    let first = service.worker_status().await;
    let first_pid = first.pid.expect("admitted worker");
    kill(first_pid);
    let restarted = format!("{GLEAM_FIXTURE}\npub fn restarted() {{\n  1\n}}\n");
    let deadline = Instant::now() + WAIT;
    loop {
        // The first call after the kill may observe the loss; the next one
        // spawns, restores and serves.
        match service
            .write(&WriteInput::new("src/main.gleam", &restarted), &session)
            .await
        {
            WorkspaceResponse::Success { .. } => break,
            WorkspaceResponse::Failure { error, .. } => {
                assert_eq!(error.kind, ErrorKind::EngineUnavailable, "{error}");
                assert!(Instant::now() < deadline, "worker never came back: {error}");
                tokio::time::sleep(Duration::from_millis(50)).await;
            }
        }
    }
    let replacement = service.worker_status().await;
    assert_ne!(replacement.pid, Some(first_pid), "a new incarnation serves");
    assert!(replacement.restarts >= 1, "{replacement:?}");
    let statuses = service.lang_pack_statuses();
    assert!(
        statuses[0].ok,
        "restored into the replacement: {statuses:?}"
    );
    assert_eq!(
        service.languages().language("src/main.gleam").as_deref(),
        Some("gleam")
    );
    assert!(
        PluginHealth::new(&dir.path().join(".magistr/workspace"))
            .read()
            .is_empty(),
        "a foreign kill is not the pack's breach"
    );

    // The replacement parse-verifies Gleam...
    let refused = service
        .write(&WriteInput::new("src/broken.gleam", BROKEN_GLEAM), &session)
        .await;
    let WorkspaceResponse::Failure { error, .. } = refused else {
        panic!("broken gleam must still be refused: {refused:?}");
    };
    assert!(
        matches!(error.kind, ErrorKind::ParseRejected { .. }),
        "{error}"
    );
    // ...and re-extracts the changed file: the stale definition row is
    // refused and asks for a refresh, the rebuild goes through the restored
    // incarnation.
    let _ = references(&service, "greet").await;
    let rebuilt = built(&service, first_build.generation).await;
    assert_eq!(
        rebuilt.parsed + rebuilt.reused,
        rebuilt.files,
        "{rebuilt:?}"
    );
    assert_eq!(
        defining_files(&service, "restarted").await,
        vec!["src/main.gleam"]
    );
    service.shutdown().await.expect("shutdown");
}

#[tokio::test]
async fn a_pack_that_fails_to_register_is_a_row_and_a_breach_and_selects_nothing() {
    let dir = workspace(Some(b"(component)"));
    let service = open(dir.path()).await;
    let statuses = service.lang_pack_statuses();
    assert_eq!(statuses.len(), 1, "{statuses:?}");
    assert!(!statuses[0].ok, "{statuses:?}");
    assert_eq!(
        service.languages().language("src/main.gleam"),
        None,
        "an unhealthy registration selects nothing"
    );
    let WorkspaceResponse::Success { notes, .. } = service.status() else {
        panic!("status");
    };
    assert!(
        notes.as_deref().unwrap_or_default().iter().any(|note| {
            note.starts_with("lang pack magistr.lang-pack-gleam: not registered (")
        }),
        "{notes:?}"
    );
    assert_eq!(
        PluginHealth::new(&dir.path().join(".magistr/workspace"))
            .read()
            .len(),
        1,
        "the failing pack is breakered"
    );
    let build = built(&service, 0).await;
    assert_eq!(build.files, 3, "{build:?}");
    assert_eq!(build.parsed, 0, "the Gleam file stays lexical-only");
    service.shutdown().await.expect("shutdown");
}

#[tokio::test]
async fn a_workspace_without_packs_never_spawns_a_worker_at_open() {
    let dir = workspace(None);
    let service = open(dir.path()).await;
    assert!(service.lang_pack_statuses().is_empty());
    assert_eq!(service.languages().language("src/main.gleam"), None);
    assert!(
        service.worker_status().await.pid.is_none() || service.index_status().generation > 0,
        "only the index's own extraction may spawn the worker"
    );
    service.shutdown().await.expect("shutdown");
}
