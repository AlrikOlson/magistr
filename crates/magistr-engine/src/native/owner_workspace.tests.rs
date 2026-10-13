use std::io::{self, BufRead, BufReader, Read, Write};
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::os::unix::net::UnixStream;
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};
use std::time::Duration;

use super::*;
use crate::native::owner_wire::Greeting;
use crate::native::{
    AttachKind, AttachRefusal, BuildIdentity, CallerIdentity, OWNER_PROTOCOL_VERSION, OwnerClient,
    OwnerIdentity, OwnerLiveness, OwnerProbe,
};

const DEADLINE: Duration = Duration::from_secs(2);

struct SessionGuard(Arc<AtomicUsize>);
impl Drop for SessionGuard {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::SeqCst);
    }
}

fn echo(active: Arc<AtomicUsize>) -> McpAcceptor {
    Arc::new(move |stream: tokio::net::UnixStream, cancel, _context| {
        let active = Arc::clone(&active);
        Box::pin(async move {
            active.fetch_add(1, Ordering::SeqCst);
            let _guard = SessionGuard(active);
            let (mut read, mut write) = stream.into_split();
            tokio::select! {
                _ = cancel.cancelled() => {},
                _ = tokio::io::copy(&mut read, &mut write) => {},
            }
        })
    })
}

async fn attach(root: &Path) -> (OwnerGeneration, UnixStream) {
    let (generation, stream) = OwnerClient::attach_workspace(
        root,
        CallerIdentity::this_process("workspace-test"),
        DEADLINE,
    )
    .await
    .expect("attached");
    let stream = stream.into_std().expect("standard stream");
    stream.set_nonblocking(false).expect("blocking fixture");
    (generation, stream)
}

fn ping(stream: &mut UnixStream) {
    stream.set_read_timeout(Some(DEADLINE)).expect("timeout");
    writeln!(stream, "ping").expect("write");
    let mut reply = String::new();
    BufReader::new(stream.try_clone().expect("reader"))
        .read_line(&mut reply)
        .expect("reply");
    assert_eq!(reply, "ping\n");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn workspace_alias_clients_share_owner_and_shutdown_joins_every_session() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let root = tmp.path().join("root");
    std::fs::create_dir(&root).expect("root");
    let alias = tmp.path().join("alias");
    std::os::unix::fs::symlink(&root, &alias).expect("alias");
    let active = Arc::new(AtomicUsize::new(0));
    let endpoint = OwnerEndpoint::serve_workspace(&root, 41, echo(Arc::clone(&active)))
        .await
        .expect("endpoint");
    let path = endpoint.path().to_path_buf();
    let (generation, mut a) = attach(&root).await;
    let (alias_generation, mut b) = attach(&alias).await;
    assert_eq!(generation.get(), 41);
    assert_eq!(alias_generation, generation);
    ping(&mut a);
    ping(&mut b);
    assert_eq!(active.load(Ordering::SeqCst), 2);
    assert!(
        !root.join(".magistr").exists(),
        "workspace attachment takes no workflow admission"
    );
    endpoint.shutdown().await.expect("joined shutdown");
    assert_eq!(
        active.load(Ordering::SeqCst),
        0,
        "no detached session cleanup"
    );
    assert!(!path.exists());
    assert_eq!(a.read(&mut [0_u8; 1]).expect("a EOF"), 0);
    assert_eq!(b.read(&mut [0_u8; 1]).expect("b EOF"), 0);
    let next = OwnerEndpoint::serve_workspace(&root, 42, echo(active))
        .await
        .expect("replacement owner");
    let (next_generation, _) = attach(&root).await;
    assert_eq!(next_generation.get(), 42);
    next.shutdown().await.expect("shutdown replacement");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn disconnecting_a_workspace_client_preserves_the_other_session() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let active = Arc::new(AtomicUsize::new(0));
    let endpoint = OwnerEndpoint::serve_workspace(tmp.path(), 1, echo(Arc::clone(&active)))
        .await
        .expect("endpoint");
    let (_, mut a) = attach(tmp.path()).await;
    let (_, mut b) = attach(tmp.path()).await;
    ping(&mut a);
    ping(&mut b);
    drop(a);
    tokio::time::timeout(DEADLINE, async {
        while active.load(Ordering::SeqCst) != 1 {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("a cleanup");
    ping(&mut b);
    endpoint.shutdown().await.expect("shutdown");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn workspace_handshake_refuses_another_root_and_workflow_identity() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let endpoint =
        OwnerEndpoint::serve_workspace(tmp.path(), 1, echo(Arc::new(AtomicUsize::new(0))))
            .await
            .expect("endpoint");
    for identity in [
        serde_json::json!({"kind":"workspace", "root":tmp.path().join("other")}),
        serde_json::json!({"kind":"workflow", "project_id":"same", "role":"service"}),
    ] {
        let mut stream = UnixStream::connect(endpoint.path()).expect("connect");
        stream.set_read_timeout(Some(DEADLINE)).expect("timeout");
        writeln!(stream, "{}", serde_json::json!({"protocol":OWNER_PROTOCOL_VERSION,"build":BuildIdentity::this_build(),"identity":identity,"attach":"mcp","caller":{"pid":1,"surface":"test"}})).expect("hello");
        let mut line = String::new();
        BufReader::new(stream)
            .read_line(&mut line)
            .expect("greeting");
        let greeting: Greeting = serde_json::from_str(line.trim()).expect("greeting JSON");
        assert!(
            matches!(greeting, Greeting::Refused(AttachRefusal::Identity { .. })),
            "{greeting:?}"
        );
    }
    endpoint.shutdown().await.expect("shutdown");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn workspace_socket_is_private_and_shutdown_preserves_replacement_files() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let endpoint =
        OwnerEndpoint::serve_workspace(tmp.path(), 1, echo(Arc::new(AtomicUsize::new(0))))
            .await
            .expect("endpoint");
    let path = endpoint.path().to_path_buf();
    assert_eq!(
        std::fs::metadata(path.parent().expect("parent"))
            .expect("directory")
            .mode()
            & 0o777,
        0o700
    );
    assert_eq!(
        std::fs::metadata(&path).expect("socket").mode() & 0o777,
        0o600
    );
    std::fs::remove_file(&path).expect("unlink our test socket");
    std::fs::write(&path, b"replacement").expect("replacement");
    endpoint.shutdown().await.expect("shutdown");
    assert_eq!(
        std::fs::read(&path).expect("preserved replacement"),
        b"replacement"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn workspace_endpoint_refuses_redirected_or_public_run_directories() {
    let tmp = tempfile::tempdir().expect("tempdir");
    // Use a short canonical root so the preferred run directory is selected.
    let root = tempfile::Builder::new()
        .prefix("mgw-")
        .tempdir_in("/tmp")
        .expect("short root");
    let state = root.path().join(".workbench");
    std::fs::create_dir(&state).expect("state");
    let run = state.join("run");
    std::fs::create_dir(&run).expect("run");
    std::fs::set_permissions(&run, std::fs::Permissions::from_mode(0o755)).expect("public mode");
    let failure =
        OwnerEndpoint::serve_workspace(root.path(), 1, echo(Arc::new(AtomicUsize::new(0))))
            .await
            .expect_err("public directory");
    assert_eq!(failure.kind(), io::ErrorKind::PermissionDenied);
    std::fs::remove_dir(&run).expect("remove run");
    std::os::unix::fs::symlink(tmp.path(), &run).expect("redirect");
    let failure =
        OwnerEndpoint::serve_workspace(root.path(), 1, echo(Arc::new(AtomicUsize::new(0))))
            .await
            .expect_err("symlink directory");
    assert_eq!(failure.kind(), io::ErrorKind::PermissionDenied);
}

fn probe(root: &Path) -> OwnerProbe {
    OwnerProbe::workspace(root, CallerIdentity::this_process("probe-test"), DEADLINE)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_probe_reports_the_live_owner_and_its_welcomed_sessions_without_being_counted() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let active = Arc::new(AtomicUsize::new(0));
    let endpoint = OwnerEndpoint::serve_workspace(tmp.path(), 41, echo(Arc::clone(&active)))
        .await
        .expect("endpoint");
    let root = tmp.path().canonicalize().expect("canonical");

    let first = tokio::task::spawn_blocking(move || probe(&root))
        .await
        .expect("probe task");
    assert_eq!(first.endpoint, endpoint.path());
    let OwnerLiveness::Live(report) = &first.liveness else {
        panic!("a live owner answers: {:?}", first.liveness);
    };
    assert_eq!(report.generation.get(), 41);
    assert_eq!(report.owner_pid, std::process::id());
    assert_eq!(report.build, BuildIdentity::this_build());
    assert_eq!(
        report.identity,
        OwnerIdentity::Workspace {
            root: tmp.path().canonicalize().expect("canonical")
        }
    );
    assert!(report.connections.is_empty(), "a probe is not a session");
    assert!(first.liveness.is_live());

    let (_, mut a) = attach(tmp.path()).await;
    ping(&mut a);
    let root = tmp.path().to_path_buf();
    let second = tokio::task::spawn_blocking(move || probe(&root))
        .await
        .expect("probe task");
    let OwnerLiveness::Live(report) = &second.liveness else {
        panic!("still live: {:?}", second.liveness);
    };
    assert_eq!(report.connections.len(), 1);
    assert_eq!(report.connections[0].attach, AttachKind::Mcp);
    assert_eq!(report.connections[0].caller.pid, std::process::id());
    assert_eq!(report.connections[0].caller.surface, "workspace-test");
    assert_eq!(report.attached(AttachKind::Mcp), 1);
    assert_eq!(report.attached(AttachKind::Commands), 0);

    drop(a);
    let root = tmp.path().to_path_buf();
    tokio::time::timeout(DEADLINE, async {
        loop {
            let root = root.clone();
            let probed = tokio::task::spawn_blocking(move || probe(&root))
                .await
                .expect("probe task");
            if let OwnerLiveness::Live(report) = &probed.liveness
                && report.connections.is_empty()
            {
                return;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("the count falls once the session ends");

    endpoint.shutdown().await.expect("shutdown");
    let root = tmp.path().to_path_buf();
    let gone = tokio::task::spawn_blocking(move || probe(&root))
        .await
        .expect("probe task");
    assert!(
        matches!(gone.liveness, OwnerLiveness::Absent),
        "{:?}",
        gone.liveness
    );
    assert!(!gone.liveness.is_live());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_probe_never_certifies_liveness_from_a_socket_file_or_a_silent_listener() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let root = tmp.path().canonicalize().expect("canonical");
    let path = crate::native::workspace_socket_path(&root).expect("endpoint path");
    std::fs::create_dir_all(path.parent().expect("parent")).expect("run dir");
    // A socket file nothing listens on: what a killed owner leaves behind.
    drop(std::os::unix::net::UnixListener::bind(&path).expect("bind"));
    assert!(path.exists());
    let stale = probe(&root);
    assert!(
        matches!(&stale.liveness, OwnerLiveness::Unanswered { .. }),
        "{:?}",
        stale.liveness
    );
    assert!(!stale.liveness.is_live());
    std::fs::remove_file(&path).expect("unlink");

    // A listener that accepts and never greets: not certified live either.
    let listener = tokio::net::UnixListener::bind(&path).expect("silent listener");
    let silent = tokio::spawn(async move {
        let mut held = Vec::new();
        while let Ok((stream, _)) = listener.accept().await {
            held.push(stream);
        }
    });
    let deadline = Duration::from_millis(150);
    let probed = tokio::task::spawn_blocking(move || {
        OwnerProbe::workspace(&root, CallerIdentity::this_process("probe-test"), deadline)
    })
    .await
    .expect("probe task");
    assert!(
        matches!(probed.liveness, OwnerLiveness::Silent { deadline: limit } if limit == deadline),
        "{:?}",
        probed.liveness
    );
    assert!(!probed.liveness.is_live());
    silent.abort();
    let _ = std::fs::remove_file(&path);
}
