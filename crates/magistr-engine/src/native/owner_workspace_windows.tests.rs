use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};
use std::time::Duration;

use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::windows::named_pipe::{ClientOptions, ServerOptions};

use super::*;
use crate::native::owner_wire::Greeting;
use crate::native::{
    AttachError, AttachRefusal, BuildIdentity, CallerIdentity, LocalStream, OWNER_PROTOCOL_VERSION,
    OwnerClient, user_sid,
};

const DEADLINE: Duration = Duration::from_secs(2);

struct SessionGuard(Arc<AtomicUsize>);
impl Drop for SessionGuard {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::SeqCst);
    }
}

fn echo(active: Arc<AtomicUsize>) -> McpAcceptor {
    Arc::new(move |stream: LocalStream, cancel| {
        let active = Arc::clone(&active);
        Box::pin(async move {
            active.fetch_add(1, Ordering::SeqCst);
            let _guard = SessionGuard(active);
            let (mut read, mut write) = tokio::io::split(stream);
            tokio::select! {
                _ = cancel.cancelled() => {},
                _ = tokio::io::copy(&mut read, &mut write) => {},
            }
        })
    })
}

async fn attach(root: &Path) -> (OwnerGeneration, LocalStream) {
    OwnerClient::attach_workspace(
        root,
        CallerIdentity::this_process("workspace-test"),
        DEADLINE,
    )
    .await
    .expect("attached")
}

async fn ping(stream: &mut LocalStream) {
    stream.write_all(b"ping\n").await.expect("write");
    let mut reply = String::new();
    tokio::time::timeout(DEADLINE, BufReader::new(stream).read_line(&mut reply))
        .await
        .expect("in time")
        .expect("reply");
    assert_eq!(reply, "ping\n");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn workspace_clients_share_one_owner_over_a_pipe_and_shutdown_ends_them() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let active = Arc::new(AtomicUsize::new(0));
    let endpoint = OwnerEndpoint::serve_workspace(tmp.path(), 41, echo(Arc::clone(&active)))
        .await
        .expect("endpoint");
    assert!(
        endpoint.path().to_string_lossy().starts_with(r"\\.\pipe\"),
        "{}",
        endpoint.path().display()
    );
    let (generation, mut a) = attach(tmp.path()).await;
    let (again, mut b) = attach(tmp.path()).await;
    assert_eq!(generation.get(), 41);
    assert_eq!(again, generation);
    ping(&mut a).await;
    ping(&mut b).await;
    assert_eq!(active.load(Ordering::SeqCst), 2);
    endpoint.shutdown().await.expect("joined shutdown");
    assert_eq!(
        active.load(Ordering::SeqCst),
        0,
        "no detached session cleanup"
    );
    let gone = OwnerClient::attach_workspace(
        tmp.path(),
        CallerIdentity::this_process("workspace-test"),
        Duration::from_millis(200),
    )
    .await;
    assert!(matches!(gone, Err(AttachError::Unavailable(_))), "{gone:?}");
    let next = OwnerEndpoint::serve_workspace(tmp.path(), 42, echo(active))
        .await
        .expect("a replacement owner takes the name once the first is gone");
    let (next_generation, _) = attach(tmp.path()).await;
    assert_eq!(next_generation.get(), 42);
    next.shutdown().await.expect("shutdown replacement");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_second_owner_on_a_live_workspace_is_refused() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let endpoint =
        OwnerEndpoint::serve_workspace(tmp.path(), 1, echo(Arc::new(AtomicUsize::new(0))))
            .await
            .expect("endpoint");
    let second =
        OwnerEndpoint::serve_workspace(tmp.path(), 2, echo(Arc::new(AtomicUsize::new(0)))).await;
    let error = second.expect_err("the name is taken");
    assert_eq!(error.kind(), io::ErrorKind::AddrInUse, "{error}");
    let (generation, _) = attach(tmp.path()).await;
    assert_eq!(generation.get(), 1, "the first owner still serves");
    endpoint.shutdown().await.expect("shutdown");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_squatted_pipe_name_is_refused_and_left_alone() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let name = workspace_socket_path(tmp.path()).expect("name");
    let squatter = ServerOptions::new()
        .first_pipe_instance(true)
        .create(&name)
        .expect("squatter");
    let refused =
        OwnerEndpoint::serve_workspace(tmp.path(), 1, echo(Arc::new(AtomicUsize::new(0)))).await;
    let error = refused.expect_err("the squatter holds the first instance");
    assert_eq!(error.kind(), io::ErrorKind::AddrInUse, "{error}");
    let client = ClientOptions::new()
        .open(&name)
        .expect("squatter still serves");
    squatter.connect().await.expect("squatter connected");
    drop(client);
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
        let mut stream = ClientOptions::new().open(endpoint.path()).expect("connect");
        let hello = serde_json::json!({"protocol":OWNER_PROTOCOL_VERSION,"build":BuildIdentity::this_build(),"identity":identity,"attach":"mcp","caller":{"pid":1,"surface":"test"}});
        stream
            .write_all(format!("{hello}\n").as_bytes())
            .await
            .expect("hello");
        let mut line = String::new();
        tokio::time::timeout(DEADLINE, BufReader::new(stream).read_line(&mut line))
            .await
            .expect("in time")
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
async fn the_pipe_grants_access_to_the_owners_user_alone() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let endpoint =
        OwnerEndpoint::serve_workspace(tmp.path(), 1, echo(Arc::new(AtomicUsize::new(0))))
            .await
            .expect("endpoint");
    let client = ClientOptions::new().open(endpoint.path()).expect("connect");
    let dacl = user_sid::pipe_dacl(&client).expect("the pipe's DACL");
    let user = user_sid::current_user().expect("user");
    assert!(
        dacl.starts_with("D:P"),
        "protected from inheritance: {dacl}"
    );
    assert_eq!(dacl.matches("(A;").count(), 1, "exactly one grant: {dacl}");
    assert!(
        dacl.ends_with(&format!(";;;{user})")),
        "granted to {user}: {dacl}"
    );
    drop(client);
    endpoint.shutdown().await.expect("shutdown");
}
