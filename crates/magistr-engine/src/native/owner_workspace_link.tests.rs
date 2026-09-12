use super::*;

use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    net::UnixListener,
};

#[tokio::test]
async fn a_slow_greeting_cannot_renew_the_workspace_attach_deadline() {
    let tmp = tempfile::Builder::new()
        .prefix("mgw-link-")
        .tempdir_in("/tmp")
        .expect("short fixture root");
    let path = workspace_socket_path(tmp.path()).expect("socket");
    std::fs::create_dir_all(path.parent().expect("parent")).expect("parent");
    let listener = UnixListener::bind(&path).expect("fake owner");
    let slow = tokio::spawn(async move {
        let (stream, _) = listener.accept().await.expect("accept");
        let mut reader = BufReader::new(stream);
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
    std::fs::remove_file(path).expect("remove fixture socket");
}
