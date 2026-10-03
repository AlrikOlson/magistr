use super::*;

use tokio::io::{AsyncReadExt, AsyncWriteExt};

#[cfg(unix)]
#[tokio::test]
async fn a_unix_stream_pair_carries_bytes_both_ways() {
    let (mut a, mut b): (LocalStream, LocalStream) = tokio::net::UnixStream::pair().expect("pair");
    a.write_all(b"ping").await.expect("write");
    let mut buf = [0_u8; 4];
    b.read_exact(&mut buf).await.expect("read");
    assert_eq!(&buf, b"ping");
}

#[cfg(windows)]
#[tokio::test]
async fn both_ends_of_a_pipe_carry_bytes_both_ways_and_shut_down() {
    use tokio::net::windows::named_pipe::{ClientOptions, ServerOptions};
    let tmp = tempfile::tempdir().expect("tempdir");
    let name =
        crate::native::owner_pipe::pipe_name("stream", tmp.path().as_os_str().as_encoded_bytes())
            .expect("name");
    let server = ServerOptions::new()
        .first_pipe_instance(true)
        .create(&name)
        .expect("server");
    let client = ClientOptions::new().open(&name).expect("client");
    server.connect().await.expect("connected");
    let mut server = LocalStream::Server(server);
    let mut client = LocalStream::Client(client);

    client.write_all(b"ping").await.expect("client write");
    client.flush().await.expect("client flush");
    let mut buf = [0_u8; 4];
    server.read_exact(&mut buf).await.expect("server read");
    assert_eq!(&buf, b"ping");

    server.write_all(b"pong").await.expect("server write");
    client.read_exact(&mut buf).await.expect("client read");
    assert_eq!(&buf, b"pong");

    server.shutdown().await.expect("shutdown is accepted");
    drop(server);
    assert_eq!(
        client.read(&mut buf).await.expect("end of stream"),
        0,
        "the client sees the owner's end go away"
    );
}
