use super::*;

use tokio::io::{AsyncReadExt, AsyncWriteExt};

fn unique(label: &str) -> PathBuf {
    let tmp = tempfile::tempdir().expect("tempdir");
    pipe_name(label, tmp.path().as_os_str().as_encoded_bytes()).expect("name")
}

#[test]
fn a_name_is_stable_per_key_and_differs_across_keys() {
    let a = pipe_name("test", b"one").expect("name");
    assert_eq!(a, pipe_name("test", b"one").expect("name"));
    assert_ne!(a, pipe_name("test", b"two").expect("name"));
    assert_ne!(a, pipe_name("other", b"one").expect("name"));
    assert!(a.to_string_lossy().starts_with(r"\\.\pipe\magistr-test-"));
}

#[tokio::test]
async fn the_first_instance_claims_the_name_and_a_second_listener_is_refused() {
    let name = unique("claim");
    let user = user_sid::current_user().expect("user");
    let _first = PipeListener::bind(&name, &user).expect("first");
    let Err(error) = PipeListener::bind(&name, &user) else {
        panic!("a live name is refused");
    };
    assert_eq!(error.kind(), io::ErrorKind::AddrInUse, "{error}");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn back_to_back_clients_always_find_an_instance_waiting() {
    let name = unique("ring");
    let user = user_sid::current_user().expect("user");
    let mut listener = PipeListener::bind(&name, &user).expect("listener");
    let server = tokio::spawn(async move {
        for _ in 0..5 {
            let mut accepted = listener.accept().await.expect("accept");
            tokio::spawn(async move {
                let mut byte = [0_u8; 1];
                accepted.read_exact(&mut byte).await.expect("read");
                accepted.write_all(&byte).await.expect("echo");
            });
        }
    });
    for round in 0..5_u8 {
        let mut client = tokio::time::timeout(Duration::from_secs(2), connect(&name))
            .await
            .expect("in time")
            .expect("connected");
        client.write_all(&[round]).await.expect("write");
        let mut byte = [0_u8; 1];
        client.read_exact(&mut byte).await.expect("echo");
        assert_eq!(byte, [round]);
    }
    server.await.expect("server");
}

#[tokio::test]
async fn nothing_serving_the_name_is_not_found() {
    let error = connect(&unique("absent"))
        .await
        .expect_err("nobody serves it");
    assert_eq!(error.kind(), io::ErrorKind::NotFound, "{error}");
}
