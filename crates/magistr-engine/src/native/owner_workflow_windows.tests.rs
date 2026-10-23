use std::sync::Arc;
use std::time::Duration;

use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

use super::*;
use crate::native::{
    AttachError, CallerIdentity, LocalStream, NativeConfig, NativeReader, OwnerClient,
    OwnerCommand, OwnerIdentity, OwnerLiveness, OwnerProbe, ProjectAuthority, RoadmapCommand,
    WriterRole, owner_socket_path,
};
use magistr_workflow::roadmap::domain::ChunkStatus;

const DEADLINE: Duration = Duration::from_secs(2);

fn config(tmp: &tempfile::TempDir) -> NativeConfig {
    let root = tmp.path().join("proj");
    std::fs::create_dir_all(&root).expect("project root");
    NativeConfig::from_lookup(root, |_| None)
        .with_data_dir(tmp.path().join("data"))
        .with_runtime_dir(tmp.path().join("run"))
}

fn host(cfg: &NativeConfig) -> NativeEngines {
    ProjectAuthority::admit_for_project(cfg, "lease".into(), WriterRole::Host)
        .expect("host admitted")
        .open()
}

fn client(cfg: &NativeConfig) -> Result<OwnerClient, AttachError> {
    OwnerClient::attach_with(
        cfg,
        "lease".into(),
        WriterRole::Host,
        CallerIdentity::this_process("test"),
        DEADLINE,
    )
}

fn add_chunk(id: &str) -> OwnerCommand {
    OwnerCommand::Roadmap(RoadmapCommand::AddChunk {
        id: id.into(),
        title: format!("Chunk {id}"),
        name: String::new(),
        status: ChunkStatus::Pending,
        priority: 100,
        description: String::new(),
        acceptance: vec![],
        deps: vec![],
        content: None,
        group: None,
    })
}

fn echo() -> McpAcceptor {
    Arc::new(|stream: LocalStream, cancel, _engines| {
        Box::pin(async move {
            let (mut read, mut write) = tokio::io::split(stream);
            tokio::select! {
                _ = cancel.cancelled() => {},
                _ = tokio::io::copy(&mut read, &mut write) => {},
            }
        })
    })
}

#[test]
fn the_workflow_endpoint_is_a_pipe_private_to_its_lock() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let cfg = config(&tmp);
    let host = owner_socket_path(&cfg, "lease", WriterRole::Host);
    assert!(
        host.to_string_lossy()
            .starts_with(r"\\.\pipe\magistr-workflow-"),
        "{}",
        host.display()
    );
    assert_eq!(host, owner_socket_path(&cfg, "lease", WriterRole::Host));
    assert_ne!(host, owner_socket_path(&cfg, "other", WriterRole::Host));
}

// The client is synchronous; calling it from inside a runtime is what the
// cockpit and the doctor do, so the test does too.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_synchronous_client_commands_the_workflow_owner_over_its_pipe() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let cfg = config(&tmp);
    let owner = host(&cfg);
    let endpoint = OwnerEndpoint::serve(owner).await.expect("endpoint");

    let mut attached = client(&cfg).expect("attached");
    assert_eq!(attached.path(), endpoint.path());
    assert_eq!(attached.generation(), endpoint.generation());
    assert_eq!(attached.owner_pid(), std::process::id());
    attached.command(add_chunk("over-a-pipe")).expect("command");
    assert!(
        NativeReader::open_for_project(&cfg, "lease".into())
            .roadmap_chunk("over-a-pipe")
            .unwrap()
            .is_some()
    );
    assert_eq!(
        attached.reconnect().expect("reconnected"),
        endpoint.generation()
    );
    attached
        .command(add_chunk("after-reconnect"))
        .expect("command");

    let probe = OwnerProbe::at(
        endpoint.path(),
        OwnerIdentity::Workflow {
            project_id: "lease".into(),
            role: WriterRole::Host,
        },
        CallerIdentity::this_process("probe-test"),
        DEADLINE,
    );
    assert!(
        matches!(probe.liveness, OwnerLiveness::Live(_)),
        "{:?}",
        probe.liveness
    );

    attached.detach();
    endpoint.shutdown().await.expect("joined shutdown");
    assert!(matches!(
        attached.reconnect(),
        Err(AttachError::Unavailable(_))
    ));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn an_mcp_session_attaches_to_the_workflow_owner_over_its_pipe() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let cfg = config(&tmp);
    let endpoint = OwnerEndpoint::serve_with(host(&cfg), Some(echo()))
        .await
        .expect("endpoint");

    let (generation, mut stream) = OwnerClient::attach_mcp(
        &cfg,
        "lease".into(),
        WriterRole::Host,
        CallerIdentity::this_process("mcp-test"),
        DEADLINE,
    )
    .await
    .expect("attached");
    assert_eq!(generation, endpoint.generation());
    stream.write_all(b"ping\n").await.expect("write");
    let mut reply = String::new();
    tokio::time::timeout(DEADLINE, BufReader::new(&mut stream).read_line(&mut reply))
        .await
        .expect("in time")
        .expect("reply");
    assert_eq!(reply, "ping\n");

    endpoint.shutdown().await.expect("joined shutdown");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn no_owner_is_unavailable_not_found() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let cfg = config(&tmp);
    match client(&cfg) {
        Err(AttachError::Unavailable(error)) => {
            assert_eq!(error.kind(), std::io::ErrorKind::NotFound, "{error}");
        }
        other => panic!("expected no owner, got {other:?}"),
    }
}
