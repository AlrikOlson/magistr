//! Admission lifetime and identity tests through the public MCP constructor.
use std::io;
use std::path::Path;

use magistr::mcp_tools::MagistrService;
use magistr_engine::native::NativeConfig;

fn config(root: &Path, data: &Path, id: &str) -> NativeConfig {
    std::fs::create_dir_all(root.join(".think-and-ship")).expect("identity directory");
    std::fs::write(
        root.join(".think-and-ship/project.json"),
        serde_json::json!({"id":id}).to_string(),
    )
    .expect("identity");
    magistr_core::evidence::writer_config(root, |_| None).with_data_dir(data)
}

async fn refused(root: &Path, config: &NativeConfig) -> io::Error {
    match MagistrService::new(root.to_path_buf(), config).await {
        Ok(_) => panic!("independent owner must be refused"),
        Err(error) => error,
    }
}

#[tokio::test]
async fn last_clone_releases_a_stable_owner_lock_and_store_changes_cannot_bypass_it() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let root = tmp.path().join("project");
    let cfg = config(&root, &tmp.path().join("data"), "lease");
    let owner = MagistrService::new(root.clone(), &cfg)
        .await
        .expect("owner");
    let clone = owner.clone();
    let lock = cfg
        .data_dir()
        .join("roadmap/sessions/lease.project-owner.lock");
    assert!(lock.is_file());
    drop(owner);
    assert_eq!(refused(&root, &cfg).await.kind(), io::ErrorKind::WouldBlock);

    let other_store = cfg.clone().with_data_dir(tmp.path().join("other-data"));
    assert_eq!(
        refused(&root, &other_store).await.kind(),
        io::ErrorKind::WouldBlock
    );
    drop(clone);
    assert!(lock.is_file(), "never unlink the lock inode on release");
    let reopened = MagistrService::new(root, &cfg)
        .await
        .expect("released lease");
    drop(reopened);
}

#[tokio::test]
async fn native_identity_coordinates_copied_roots_and_separates_projects() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let data = tmp.path().join("data");
    let root = tmp.path().join("one");
    let copy = tmp.path().join("copy");
    let other = tmp.path().join("other");
    let cfg = config(&root, &data, "same");
    let copied = config(&copy, &data, "same");
    let distinct = config(&other, &data, "different");
    let owner = MagistrService::new(root, &cfg).await.expect("owner");
    assert_eq!(
        refused(&copy, &copied).await.kind(),
        io::ErrorKind::WouldBlock
    );
    let independent = MagistrService::new(other, &distinct)
        .await
        .expect("different project");
    drop((owner, independent));
}

#[cfg(unix)]
#[tokio::test]
async fn symlinked_project_and_data_paths_cannot_evade_owner_admission() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let root = tmp.path().join("project");
    let data = tmp.path().join("data");
    let cfg = config(&root, &data, "alias");
    let owner = MagistrService::new(root.clone(), &cfg)
        .await
        .expect("owner");
    let alias = tmp.path().join("alias");
    let data_alias = tmp.path().join("data-alias");
    std::os::unix::fs::symlink(&root, &alias).expect("project alias");
    std::os::unix::fs::symlink(&data, &data_alias).expect("data alias");
    let aliased =
        magistr_core::evidence::writer_config(&alias, |_| None).with_data_dir(&data_alias);
    assert_eq!(
        refused(&alias, &aliased).await.kind(),
        io::ErrorKind::WouldBlock
    );
    drop(owner);
}

#[tokio::test]
async fn stateless_servers_still_exclude_duplicate_project_owners() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let root = tmp.path().join("project");
    let cfg = config(&root, &tmp.path().join("data"), "stateless").persist(false);
    let owner = MagistrService::new(root.clone(), &cfg)
        .await
        .expect("owner");
    assert_eq!(refused(&root, &cfg).await.kind(), io::ErrorKind::WouldBlock);
    assert!(
        !cfg.data_dir().exists(),
        "no persistent native store in stateless mode"
    );
    drop(owner);
}

#[tokio::test]
async fn owner_lock_io_failure_is_reported_before_any_engines_are_opened() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let root = tmp.path().join("project");
    let data = tmp.path().join("data");
    let cfg = config(&root, &data, "broken");
    std::fs::write(&data, "existing file").expect("blocking path");
    let error = refused(&root, &cfg).await;
    assert!(error.to_string().contains("project owner"), "{error}");
    assert_eq!(
        std::fs::read_to_string(&data).expect("unchanged file"),
        "existing file"
    );
}

#[cfg(not(unix))]
#[tokio::test]
async fn explicit_unix_feed_is_refused_on_unsupported_platforms() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let root = tmp.path().join("project");
    let cfg = config(&root, &tmp.path().join("data"), "unsupported")
        .with_broadcast_path(Some(tmp.path().join("feed.sock")));
    assert_eq!(
        refused(&root, &cfg).await.kind(),
        io::ErrorKind::Unsupported
    );
}

#[cfg(unix)]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_host_started_owner_publishes_existing_execution_views() {
    use magistr::application::workflow_client::EngineClient;
    use magistr_workflow::ship::domain::task::TaskType;
    use tokio::io::{AsyncBufReadExt as _, BufReader};
    use tokio::time::{Duration, timeout};

    let tmp = tempfile::tempdir().expect("temporary");
    let root = tmp.path().join("project");
    let cfg = config(&root, &tmp.path().join("data"), "host-feed")
        .with_broadcast_path(Some(tmp.path().join("feed.sock")));
    let host = EngineClient::with_config(cfg.clone()).expect("host owner");
    let writer = host.writer().expect("writer");
    let task_view = writer
        .for_working_copy(&root, "task")
        .expect("execution before serving");
    let endpoint = host.serve_owner().await.expect("serve").expect("supported");
    let stream = tokio::net::UnixStream::connect(cfg.feed_socket_path())
        .await
        .expect("host feed");
    assert!(
        writer
            .authority()
            .feed()
            .expect("publisher")
            .wait_for_subscribers(1, Duration::from_secs(2))
            .await
    );
    task_view
        .ship_set_objective("A task".into(), vec![], vec![], "task".into())
        .expect("objective");
    task_view
        .ship_add_task(
            "verify".into(),
            "Verify changes".into(),
            TaskType::Test,
            None,
        )
        .expect("task");
    let mut lines = BufReader::new(stream).lines();
    let frame: serde_json::Value = serde_json::from_str(
        &timeout(Duration::from_secs(2), lines.next_line())
            .await
            .expect("bounded event")
            .expect("read")
            .expect("frame"),
    )
    .expect("frame JSON");
    assert_eq!(frame["family"], "ship");
    assert_eq!(
        frame["execution_key"],
        task_view.config().execution_context().key()
    );
    assert_eq!(
        frame["epoch"].as_u64(),
        Some(writer.authority().generation().get())
    );
    endpoint.shutdown().await.expect("bounded endpoint cleanup");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_host_attaches_to_the_service_owner_and_reads_shared_state() {
    use magistr::application::workflow_client::EngineClient;
    use magistr_engine::native::{NativeReader, ProjectAuthority, WriterRole};
    let tmp = tempfile::tempdir().expect("temporary");
    let root = tmp.path().join("project");
    let cfg = config(&root, &tmp.path().join("data"), "beside");
    let owner = MagistrService::new(root.clone(), &cfg)
        .await
        .expect("owner");
    let endpoint = owner
        .serve_owner_endpoint()
        .await
        .expect("endpoint")
        .expect("supported");
    assert_eq!(
        ProjectAuthority::admit(&cfg, WriterRole::Host)
            .expect_err("one writer")
            .kind(),
        io::ErrorKind::WouldBlock
    );
    let host = EngineClient::admit_or_attach_config(cfg.clone()).expect("attached host");
    assert!(host.reader().roadmap_chunks().is_empty());
    drop(host);
    assert_eq!(refused(&root, &cfg).await.kind(), io::ErrorKind::WouldBlock);
    assert!(NativeReader::open(&cfg).roadmap_chunks().is_empty());
    endpoint.shutdown().await.expect("bounded cleanup");
    drop(owner);
    ProjectAuthority::admit(&cfg, WriterRole::Host).expect("released project");
}
