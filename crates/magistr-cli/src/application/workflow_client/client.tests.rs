use std::sync::Arc;

use magistr_engine::NativeConfig;
use magistr_engine::native::{EvidencePort, GateContract, SourceWitness};

use super::*;

/// The shape of the capability a host attaches; what it answers is not
/// this crate's concern.
struct NoEvidence;

impl EvidencePort for NoEvidence {
    fn observe(&self) -> Result<Box<dyn SourceWitness>, String> {
        Err("not observed in this test".into())
    }
    fn required_gates(&self) -> Result<Vec<GateContract>, String> {
        Ok(Vec::new())
    }
}

fn config(root: &Path, data: &Path) -> NativeConfig {
    NativeConfig::from_lookup(root, |_| None)
        .with_data_dir(data)
        .with_evidence(Arc::new(NoEvidence))
}

#[test]
fn a_writer_config_without_evidence_is_refused_before_the_lease() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let root = tmp.path().join("proj");
    std::fs::create_dir_all(&root).expect("root");
    let bare = NativeConfig::from_lookup(&root, |_| None).with_data_dir(tmp.path().join("data"));
    let err = EngineClient::with_config(bare.clone()).expect_err("refused");
    assert!(
        matches!(err, EngineError::Admission { root: ref refused, ref source }
            if *refused == root && source.kind() == io::ErrorKind::InvalidInput),
        "{err}"
    );
    assert!(err.to_string().contains("evidence capability"));
    // The lease was never taken: a proper host still admits.
    EngineClient::with_config(config(&root, &tmp.path().join("data"))).expect("opens");
}

#[test]
fn opens_on_a_project_directory_and_creates_the_store() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let root = tmp.path().join("proj");
    std::fs::create_dir_all(&root).expect("root");
    let data = tmp.path().join("data");
    let client = EngineClient::with_config(config(&root, &data)).expect("opens");
    assert!(data.is_dir(), "the store directory exists");
    assert!(!client.reader().project_id().is_empty());
    assert!(client.writer().is_some(), "opened as the host");
    assert!(client.socket().to_string_lossy().ends_with(".sock"));
}

#[test]
fn a_missing_root_is_refused_before_anything_else() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let root = tmp.path().join("nowhere");
    let err =
        EngineClient::with_config(config(&root, &tmp.path().join("data"))).expect_err("refused");
    assert!(
        matches!(err, EngineError::NoProject(ref p) if *p == root),
        "{err}"
    );
    assert!(err.to_string().contains("not a project directory"));
}

#[test]
fn an_uncreatable_store_is_reported_with_its_path() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let root = tmp.path().join("proj");
    std::fs::create_dir_all(&root).expect("root");
    // A file where the store directory should go.
    let blocker = tmp.path().join("blocker");
    std::fs::write(&blocker, b"x").expect("file");
    let data = blocker.join("data");
    let err = EngineClient::with_config(config(&root, &data)).expect_err("refused");
    assert!(
        matches!(err, EngineError::Store { ref path, .. } if *path == data),
        "{err}"
    );
    assert!(std::error::Error::source(&err).is_some());
}

#[test]
fn a_non_persistent_store_needs_no_directory() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let root = tmp.path().join("proj");
    std::fs::create_dir_all(&root).expect("root");
    let data = tmp.path().join("data");
    EngineClient::with_config(config(&root, &data).persist(false)).expect("opens");
    assert!(!data.exists());
}

#[test]
fn a_second_host_on_the_same_project_is_refused_until_the_first_is_gone() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let root = tmp.path().join("proj");
    std::fs::create_dir_all(&root).expect("root");
    let data = tmp.path().join("data");
    let first = EngineClient::with_config(config(&root, &data)).expect("the host");
    let err = EngineClient::with_config(config(&root, &data)).expect_err("a second host");
    assert!(
        matches!(err, EngineError::Admission { root: ref refused, .. } if *refused == root),
        "{err}"
    );
    assert!(
        matches!(&err, EngineError::Admission { source, .. } if source.kind() == io::ErrorKind::WouldBlock),
        "{err}"
    );
    assert!(std::error::Error::source(&err).is_some());

    // A cockpit beside a loop attaches to the loop's admitted writer rather
    // than admitting a second one; both see the same store and socket.
    let attached = EngineClient::attach(first.writer().expect("the host").clone());
    assert_eq!(attached.socket(), first.socket());
    assert_eq!(attached.reader().project_id(), first.reader().project_id());
    // An observer beside the host needs no lease and holds no writer.
    let viewer = EngineClient::observe_config(config(&root, &data)).expect("an observer");
    assert!(viewer.writer().is_none());
    assert_eq!(viewer.reader().project_id(), first.reader().project_id());
    drop(first);
    // The attached clone still carries the lease.
    EngineClient::with_config(config(&root, &data)).expect_err("still held by the attached clone");
    drop(attached);
    EngineClient::with_config(config(&root, &data)).expect("released with the last writer");
}

/// A pending chunk on the host's writer.
#[cfg(unix)]
fn seed_chunk(host: &EngineClient, id: &str) {
    host.writer()
        .expect("the host")
        .roadmap_add_chunk(
            id.into(),
            format!("Chunk {id}"),
            String::new(),
            magistr_engine::ChunkStatus::Pending,
            100,
            String::new(),
            vec![],
            vec![],
            None,
            None,
        )
        .expect("chunk");
}

/// The store under a tempdir is too deep for `sun_path`: the endpoint
/// lands in the runtime directory, and `/tmp` always fits.
#[cfg(unix)]
fn attachable(root: &Path, data: &Path) -> NativeConfig {
    config(root, data).with_runtime_dir("/tmp")
}

#[cfg(unix)]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_second_cockpit_attaches_to_the_running_host_and_acts_through_it() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let root = tmp.path().join("proj");
    std::fs::create_dir_all(&root).expect("root");
    let data = tmp.path().join("data");
    let host = EngineClient::with_config(attachable(&root, &data)).expect("the host");
    seed_chunk(&host, "c1");
    let endpoint = host
        .serve_owner()
        .await
        .expect("served")
        .expect("a host serves an endpoint");

    let second = EngineClient::admit_or_attach_config(attachable(&root, &data))
        .expect("attached to the running host");
    assert!(second.writer().is_none(), "no second writer");
    assert_eq!(second.attached_generation(), Some(endpoint.generation()));
    let mut port = second.port().expect("an attached port");
    let started = port.start_chunk("c1").expect("acts through the host");
    assert_eq!(started.status, magistr_engine::ChunkStatus::InProgress);
    assert_eq!(
        host.reader()
            .roadmap_chunk("c1")
            .unwrap()
            .map(|chunk| chunk.status),
        Some(magistr_engine::ChunkStatus::InProgress),
        "the host's own writer carried it out"
    );
    assert!(
        port.start_chunk("missing")
            .expect_err("refused")
            .contains("missing"),
        "the engine's refusal comes back in its own words"
    );
    // Still one lease, and observers still need none.
    EngineClient::with_config(attachable(&root, &data)).expect_err("held by the host");
    let viewer = EngineClient::observe_config(attachable(&root, &data)).expect("observer");
    assert!(viewer.port().is_none() && viewer.attached_generation().is_none());
    assert!(
        viewer
            .serve_owner()
            .await
            .expect("nothing to serve")
            .is_none(),
        "an observer serves no endpoint"
    );
    drop(endpoint);
}

#[cfg(unix)]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_held_host_without_an_endpoint_fails_closed() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let root = tmp.path().join("proj");
    std::fs::create_dir_all(&root).expect("root");
    let data = tmp.path().join("data");
    let host = EngineClient::with_config(attachable(&root, &data)).expect("the host");
    let err = EngineClient::admit_or_attach_config(attachable(&root, &data))
        .expect_err("held, and nothing to attach to");
    assert!(
        matches!(
            &err,
            EngineError::Occupied { root: refused, lease, attach: magistr_engine::AttachError::Unavailable(_) }
                if *refused == root && lease.kind() == std::io::ErrorKind::WouldBlock
        ),
        "{err}"
    );
    assert!(err.to_string().contains("observer cockpit"), "{err}");
    assert!(std::error::Error::source(&err).is_some());
    drop(host);
    let host = EngineClient::admit_or_attach_config(attachable(&root, &data))
        .expect("the free role is taken outright");
    assert!(host.writer().is_some() && host.attached_generation().is_none());
}

#[cfg(unix)]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn an_attached_port_reports_a_gone_owner_and_opens_nothing() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let root = tmp.path().join("proj");
    std::fs::create_dir_all(&root).expect("root");
    let data = tmp.path().join("data");
    let host = EngineClient::with_config(attachable(&root, &data)).expect("the host");
    seed_chunk(&host, "c1");
    let endpoint = host.serve_owner().await.expect("served");
    let second = EngineClient::admit_or_attach_config(attachable(&root, &data)).expect("attached");
    let mut port = second.port().expect("port");
    port.pin(1, true)
        .expect_err("no such step, but the owner answered");

    // The host shuts down: endpoint first, then its writer.
    drop(endpoint);
    drop(host);
    let gone = port.start_chunk("c1").expect_err("hung up");
    assert!(gone.contains("gone"), "{gone}");
    let nobody = port.start_chunk("c1").expect_err("nothing to reconnect to");
    assert!(nobody.contains("no owner endpoint"), "{nobody}");
    assert!(second.writer().is_none(), "never a writer of its own");
    EngineClient::with_config(attachable(&root, &data))
        .expect("the role is free: the attached cockpit held nothing");
}

#[cfg(not(unix))]
#[tokio::test]
async fn a_host_off_unix_serves_no_endpoint_and_still_starts() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let root = tmp.path().join("proj");
    std::fs::create_dir_all(&root).expect("root");
    let host = EngineClient::with_config(config(&root, &tmp.path().join("data"))).expect("host");
    assert!(
        host.serve_owner()
            .await
            .expect("the session starts without an endpoint")
            .is_none()
    );
}
