//! Admission lifetime and identity tests through the public MCP constructor.
use std::io;
use std::path::Path;

use magistr::mcp_tools::MagistrService;
use magistr_engine::tas::TasConfig;

fn config(root: &Path, data: &Path, id: &str) -> TasConfig {
    std::fs::create_dir_all(root.join(".think-and-ship")).expect("identity directory");
    std::fs::write(
        root.join(".think-and-ship/project.json"),
        serde_json::json!({"id":id}).to_string(),
    )
    .expect("identity");
    TasConfig::from_lookup(root, |_| None).with_data_dir(data)
}

async fn refused(root: &Path, config: &TasConfig) -> io::Error {
    match MagistrService::new(root.to_path_buf(), config).await {
        Ok(_) => panic!("independent owner must be refused"),
        Err(error) => error,
    }
}

#[tokio::test]
async fn last_clone_releases_a_stable_owner_lock_and_distinct_stores_are_independent() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let root = tmp.path().join("project");
    let cfg = config(&root, &tmp.path().join("data"), "lease");
    let owner = MagistrService::new(root.clone(), &cfg)
        .await
        .expect("owner");
    let clone = owner.clone();
    let lock = cfg.data_dir.join("roadmap/sessions/lease.mcp-owner.lock");
    assert!(lock.is_file());
    drop(owner);
    assert_eq!(refused(&root, &cfg).await.kind(), io::ErrorKind::WouldBlock);

    let other_store = cfg.clone().with_data_dir(tmp.path().join("other-data"));
    let independent = MagistrService::new(root.clone(), &other_store)
        .await
        .expect("other store");
    drop(independent);
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
    let aliased = TasConfig::from_lookup(&alias, |_| None).with_data_dir(&data_alias);
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
        !cfg.data_dir.exists(),
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

#[tokio::test]
async fn a_host_writer_is_admitted_beside_the_service_owner_and_readers_see_both() {
    use magistr_engine::tas::{ChunkStatus, NativeReader, ProjectAuthority, WriterRole};
    let tmp = tempfile::tempdir().expect("tempdir");
    let root = tmp.path().join("project");
    let cfg = config(&root, &tmp.path().join("data"), "beside");
    let owner = MagistrService::new(root.clone(), &cfg)
        .await
        .expect("service owner");
    // The cockpit or loop is the other role: admitted beside the service.
    let host = ProjectAuthority::admit(&cfg, WriterRole::Host)
        .expect("host beside the service")
        .open();
    assert_eq!(host.project_id(), "beside");
    host.roadmap_add_chunk(
        "from-host".into(),
        "Host mutation".into(),
        String::new(),
        ChunkStatus::Pending,
        100,
        String::new(),
        vec![],
        vec![],
        None,
        None,
    )
    .expect("host writes under its lease");
    // Neither role admits a second of itself while the first is alive.
    assert_eq!(refused(&root, &cfg).await.kind(), io::ErrorKind::WouldBlock);
    assert_eq!(
        ProjectAuthority::admit(&cfg, WriterRole::Host)
            .expect_err("second host")
            .kind(),
        io::ErrorKind::WouldBlock
    );
    // A reader needs no lease and sees the host's write on the shared store.
    let observer = NativeReader::open(&cfg);
    assert!(observer.roadmap_chunk("from-host").is_some());
    assert!(
        cfg.data_dir
            .join("roadmap/sessions/beside.host-owner.lock")
            .is_file(),
        "the host holds its own lock file beside the service's"
    );
    drop(host);
    ProjectAuthority::admit(&cfg, WriterRole::Host).expect("host released independently");
    drop(owner);
    MagistrService::new(root, &cfg)
        .await
        .expect("service released independently");
}
