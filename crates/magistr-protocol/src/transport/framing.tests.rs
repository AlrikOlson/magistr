use super::*;

use tokio::io::BufReader;

#[tokio::test]
async fn write_then_read_roundtrip() {
    let payload = b"hello world";
    let mut buf = Vec::new();
    write_frame(&mut buf, payload).await.unwrap();

    let mut reader = BufReader::new(&buf[..]);
    let result = read_frame(&mut reader).await.unwrap();
    assert_eq!(result, payload);
}

#[tokio::test]
async fn frame_format_is_content_length() {
    let payload = b"test";
    let mut buf = Vec::new();
    write_frame(&mut buf, payload).await.unwrap();

    let expected = b"Content-Length: 4\r\n\r\ntest";
    assert_eq!(buf, expected);
}

#[tokio::test]
async fn read_frame_empty_payload() {
    let raw = b"Content-Length: 0\r\n\r\n";
    let mut reader = BufReader::new(&raw[..]);
    let result = read_frame(&mut reader).await.unwrap();
    assert!(result.is_empty());
}

#[tokio::test]
async fn read_frame_missing_header_returns_error() {
    let raw = b"garbage data\r\n\r\n";
    let mut reader = BufReader::new(&raw[..]);
    let result = read_frame(&mut reader).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn read_frame_eof_returns_connection_closed() {
    let raw = b"";
    let mut reader = BufReader::new(&raw[..]);
    let result = read_frame(&mut reader).await;
    assert!(matches!(result, Err(TransportError::ConnectionClosed)));
}

#[tokio::test]
async fn multiple_frames_sequential() {
    let mut buf = Vec::new();
    write_frame(&mut buf, b"first").await.unwrap();
    write_frame(&mut buf, b"second").await.unwrap();

    let mut reader = BufReader::new(&buf[..]);
    let first = read_frame(&mut reader).await.unwrap();
    let second = read_frame(&mut reader).await.unwrap();

    assert_eq!(first, b"first");
    assert_eq!(second, b"second");
}

#[tokio::test]
async fn large_payload_roundtrip() {
    let payload: Vec<u8> = (0..10_000).map(|i| (i % 256) as u8).collect();
    let mut buf = Vec::new();
    write_frame(&mut buf, &payload).await.unwrap();

    let mut reader = BufReader::new(&buf[..]);
    let result = read_frame(&mut reader).await.unwrap();
    assert_eq!(result, payload);
}

#[tokio::test]
async fn write_frame_to_closed_writer_returns_error() {
    let mut writer = FailWriter;
    let result = write_frame(&mut writer, b"test").await;
    assert!(matches!(result, Err(TransportError::Io(_))));
}

/// A writer that always returns an error.
struct FailWriter;

impl tokio::io::AsyncWrite for FailWriter {
    fn poll_write(
        self: std::pin::Pin<&mut Self>,
        _cx: &mut std::task::Context<'_>,
        _buf: &[u8],
    ) -> std::task::Poll<std::io::Result<usize>> {
        std::task::Poll::Ready(Err(std::io::Error::new(
            std::io::ErrorKind::BrokenPipe,
            "mock failure",
        )))
    }

    fn poll_flush(
        self: std::pin::Pin<&mut Self>,
        _cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        std::task::Poll::Ready(Ok(()))
    }

    fn poll_shutdown(
        self: std::pin::Pin<&mut Self>,
        _cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<std::io::Result<()>> {
        std::task::Poll::Ready(Ok(()))
    }
}
