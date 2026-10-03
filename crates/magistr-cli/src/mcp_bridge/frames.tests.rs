use super::*;
use std::time::Duration;
use tokio::io::AsyncReadExt;

#[tokio::test]
async fn half_close_keeps_last_response_readable() {
    let (client, mut owner) = tokio::io::duplex(64);
    let mut frames = Frames::new(client);
    frames.half_close().await;
    assert_eq!(
        tokio::time::timeout(Duration::from_secs(1), owner.read(&mut [0]))
            .await
            .expect("bounded EOF")
            .expect("EOF"),
        0
    );
    owner.write_all(b"response\n").await.expect("last response");
    assert_eq!(frames.next_line().await.as_deref(), Some("response"));
    assert_eq!(
        frames.send("late").await.unwrap_err().kind(),
        std::io::ErrorKind::BrokenPipe
    );
}
