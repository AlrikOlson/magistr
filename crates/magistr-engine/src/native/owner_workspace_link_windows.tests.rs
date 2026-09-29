use super::*;

use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::windows::named_pipe::ServerOptions;

#[tokio::test]
async fn a_slow_greeting_cannot_renew_the_workspace_attach_deadline() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let name = workspace_socket_path(tmp.path()).expect("name");
    let server = ServerOptions::new()
        .first_pipe_instance(true)
        .create(&name)
        .expect("fake owner");
    let slow = tokio::spawn(async move {
        server.connect().await.expect("accept");
        let mut reader = BufReader::new(server);
        let mut line = String::new();
        reader.read_line(&mut line).await.expect("hello");
        for byte in b"{\"greeting\":\"welcome\",\"generation\":1,\"owner_pid\":1}\n" {
            if reader.get_mut().write_all(&[*byte]).await.is_err() {
                return;
            }
            tokio::time::sleep(Duration::from_millis(15)).await;
        }
    });
    let deadline = Duration::from_millis(90);
    let start = tokio::time::Instant::now();
    let result = attach(
        tmp.path(),
        CallerIdentity::this_process("deadline-test"),
        deadline,
    )
    .await;
    assert!(
        matches!(result, Err(AttachError::Deadline(limit)) if limit == deadline),
        "{result:?}"
    );
    assert!(
        start.elapsed() < Duration::from_millis(500),
        "the entire greeting shares one deadline"
    );
    slow.abort();
    let _ = slow.await;
}

#[tokio::test]
async fn nothing_listening_is_unavailable_not_a_deadline() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let result = attach(
        tmp.path(),
        CallerIdentity::this_process("absent-test"),
        Duration::from_millis(200),
    )
    .await;
    assert!(
        matches!(&result, Err(AttachError::Unavailable(error)) if error.kind() == std::io::ErrorKind::NotFound),
        "{result:?}"
    );
}
