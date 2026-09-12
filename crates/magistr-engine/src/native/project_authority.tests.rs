use std::io;

use super::*;
use magistr_workflow::roadmap::domain::ChunkStatus;

fn config(tmp: &tempfile::TempDir) -> NativeConfig {
    let root = tmp.path().join("proj");
    std::fs::create_dir_all(&root).expect("project root");
    NativeConfig::from_lookup(root, |_| None).with_data_dir(tmp.path().join("data"))
}

fn refused(config: &NativeConfig, role: WriterRole) -> io::Error {
    match ProjectAuthority::admit_for_project(config, "lease".into(), role) {
        Ok(_) => panic!("a second {role:?} writer must be refused"),
        Err(error) => error,
    }
}

fn add_chunk(engines: &NativeEngines, id: &str) {
    engines
        .roadmap_add_chunk(
            id.into(),
            format!("Chunk {id}"),
            String::new(),
            ChunkStatus::Pending,
            100,
            String::new(),
            vec![],
            vec![],
            None,
            None,
        )
        .expect("add chunk");
}

#[test]
fn one_writer_per_role_is_admitted_and_the_roles_coexist() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let cfg = config(&tmp);
    let host =
        ProjectAuthority::admit_for_project(&cfg, "lease".into(), WriterRole::Host).expect("host");
    assert_eq!(host.role(), WriterRole::Host);
    assert_eq!(host.project_id(), "lease");
    assert!(host.feed().is_none());
    // The cockpit and its agent's mcp-serve are different roles: both admitted.
    let service = ProjectAuthority::admit_for_project(&cfg, "lease".into(), WriterRole::Service)
        .expect("service beside the host");
    assert_eq!(
        refused(&cfg, WriterRole::Host).kind(),
        io::ErrorKind::WouldBlock
    );
    let error = refused(&cfg, WriterRole::Service);
    assert_eq!(error.kind(), io::ErrorKind::WouldBlock);
    assert!(error.to_string().contains("project owner"), "{error}");
    assert!(
        refused(&cfg, WriterRole::Host)
            .to_string()
            .contains("project host owner")
    );

    // Each role has its own stable lock file under the store.
    let sessions = cfg.data_dir.join("roadmap/sessions");
    assert!(sessions.join("lease.host-owner.lock").is_file());
    assert!(sessions.join("lease.mcp-owner.lock").is_file());

    // Both writers act on one store; the reader sees them without a lease.
    let host = host.open();
    let service = service.open();
    add_chunk(&host, "from-host");
    add_chunk(&service, "from-service");
    let observer = NativeReader::open_for_project(&cfg, "lease".into());
    let ids: Vec<String> = observer
        .roadmap_chunks()
        .into_iter()
        .map(|c| c.id)
        .collect();
    assert!(ids.contains(&"from-host".into()) && ids.contains(&"from-service".into()));
    drop((host, service));
}

#[test]
fn the_lease_lives_with_the_last_writer_clone_and_readers_never_hold_it() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let cfg = config(&tmp);
    let writer = ProjectAuthority::admit_for_project(&cfg, "lease".into(), WriterRole::Host)
        .expect("host")
        .open();
    let clone = writer.clone();
    let reader = writer.reader();
    drop(writer);
    // A clone still writes under the admission.
    add_chunk(&clone, "late");
    assert_eq!(
        refused(&cfg, WriterRole::Host).kind(),
        io::ErrorKind::WouldBlock
    );
    drop(clone);
    // Only the reader remains, and it holds no lock: the role is free again.
    assert_eq!(reader.roadmap_chunks().len(), 1);
    let reopened = ProjectAuthority::admit_for_project(&cfg, "lease".into(), WriterRole::Host)
        .expect("released with the last writer");
    drop(reopened);
}

#[test]
fn a_stateless_store_leases_under_the_project_root_and_writes_nothing() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let cfg = config(&tmp).persist(false);
    let host = ProjectAuthority::admit_for_project(&cfg, "lease".into(), WriterRole::Host)
        .expect("host")
        .open();
    add_chunk(&host, "ephemeral");
    assert!(cfg.project_root.join(".magistr/host-owner.lock").is_file());
    assert_eq!(
        refused(&cfg, WriterRole::Host).kind(),
        io::ErrorKind::WouldBlock
    );
    assert!(
        !cfg.data_dir.exists(),
        "no persistent store in stateless mode"
    );
    drop(host);
}

#[test]
fn a_lock_io_failure_is_reported_before_any_engine_opens() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let cfg = config(&tmp);
    std::fs::write(&cfg.data_dir, "existing file").expect("blocking path");
    let error = refused(&cfg, WriterRole::Host);
    assert_ne!(error.kind(), io::ErrorKind::WouldBlock);
    assert!(error.to_string().contains("project host owner"), "{error}");
    assert_eq!(
        std::fs::read_to_string(&cfg.data_dir).expect("unchanged file"),
        "existing file"
    );
}

#[cfg(unix)]
#[tokio::test]
async fn publishing_refuses_a_live_listener_and_reclaims_a_dead_socket() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let cfg = config(&tmp);
    let socket = tmp.path().join("feed.sock");
    let listener = std::os::unix::net::UnixListener::bind(&socket).expect("live feed");
    let error = ProjectAuthority::admit_for_project(&cfg, "lease".into(), WriterRole::Service)
        .expect("service")
        .publish(&socket)
        .await
        .expect_err("occupied publisher");
    assert_eq!(error.kind(), io::ErrorKind::AddrInUse);
    assert!(socket.exists(), "the live socket is preserved");
    // The failed publish consumed its authority and released the lease.
    drop(listener);
    let publishing = ProjectAuthority::admit_for_project(&cfg, "lease".into(), WriterRole::Service)
        .expect("released after the failed publish")
        .publish(&socket)
        .await
        .expect("a dead socket is reclaimed");
    assert!(publishing.feed().is_some());
    let (unified, engines) = publishing.serve();
    assert_eq!(engines.authority().role(), WriterRole::Service);
    assert!(engines.authority().feed().is_some());
    drop((unified, engines));
}
