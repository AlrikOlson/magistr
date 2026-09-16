use super::*;
use tokio::io::AsyncReadExt;

#[tokio::test]
async fn half_close_delivers_eof_while_preserving_the_response_reader() {
    let (client, mut server) = tokio::io::duplex(64);
    let (read, write) = tokio::io::split(client);
    let mut link = Link {
        reader: BufReader::new(Box::pin(read) as Pin<Box<dyn AsyncRead + Send>>).lines(),
        writer: Some(Box::pin(write)),
        embedded: None,
        generation: 1,
    };
    link.half_close().await;
    let mut byte = [0];
    let read = tokio::time::timeout(Duration::from_secs(1), server.read(&mut byte))
        .await
        .expect("half-close delivers EOF before the bridge's drain deadline")
        .expect("read EOF");
    assert_eq!(read, 0);
    server
        .write_all(b"last response\n")
        .await
        .expect("response");
    assert_eq!(link.next_line().await.as_deref(), Some("last response"));
    link.close().await.expect("close link");
}
