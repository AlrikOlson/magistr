use super::*;

use std::future::poll_fn;
use std::io::{Seek, SeekFrom};
use std::os::unix::net::UnixStream;

use tokio::io::{AsyncReadExt, AsyncWriteExt};

fn flags(file: &impl AsRawFd) -> libc::c_int {
    // SAFETY: every caller keeps the descriptor's owner alive.
    let flags = unsafe { libc::fcntl(file.as_raw_fd(), libc::F_GETFL) };
    assert_ne!(flags, -1);
    flags
}

#[tokio::test]
async fn cancelling_idle_input_restores_flags_while_peer_remains_open() {
    let (reader, _writer) = UnixStream::pair().expect("input pair");
    let original = flags(&reader);
    let mut input = ProxyStdio::new(&reader, Interest::READABLE).expect("input");
    let mut bytes = [0; 1];
    let pending = poll_fn(|context| {
        Poll::Ready(
            Pin::new(&mut input)
                .poll_read(context, &mut ReadBuf::new(&mut bytes))
                .is_pending(),
        )
    })
    .await;
    assert!(pending, "idle input waits on readiness");
    assert!(input.readiness.is_some());
    let empty = poll_fn(|context| {
        Poll::Ready(Pin::new(&mut input).poll_read(context, &mut ReadBuf::new(&mut [])))
    })
    .await;
    assert!(matches!(empty, Poll::Ready(Ok(()))));
    assert_ne!(flags(&reader) & libc::O_NONBLOCK, 0);
    drop(input);
    assert_eq!(
        flags(&reader),
        original,
        "cancellation restores shared flags"
    );
}

#[tokio::test]
async fn cancelling_backpressured_output_restores_flags_without_peer_drain() {
    let (writer, _reader) = UnixStream::pair().expect("output pair");
    let original = flags(&writer);
    let mut output = ProxyStdio::new(&writer, Interest::WRITABLE).expect("output");
    let bytes = [b'x'; 8192];
    for _ in 0..4096 {
        let written =
            poll_fn(|context| Poll::Ready(Pin::new(&mut output).poll_write(context, &bytes))).await;
        match written {
            Poll::Ready(Ok(count)) => assert_ne!(count, 0),
            Poll::Ready(Err(error)) => panic!("output failed: {error}"),
            Poll::Pending => {
                assert!(output.readiness.is_some());
                let empty =
                    poll_fn(|context| Poll::Ready(Pin::new(&mut output).poll_write(context, &[])))
                        .await;
                assert!(matches!(empty, Poll::Ready(Ok(0))));
                drop(output);
                // macOS adds kernel-only FWASWRITTEN after a successful
                // write; F_SETFL restores mutable flags, not that history.
                let mutable = libc::O_APPEND
                    | libc::O_ASYNC
                    | libc::O_SYNC
                    | libc::O_DSYNC
                    | libc::O_NONBLOCK;
                assert_eq!(
                    flags(&writer) & mutable,
                    original & mutable,
                    "cancellation restores mutable shared flags"
                );
                return;
            }
        }
    }
    panic!("unread local socket should backpressure before 32 MiB");
}

#[tokio::test]
async fn redirected_file_and_null_io_need_no_reactor_registration() {
    let mut file = tempfile::tempfile().expect("regular file");
    file.write_all(b"redirected input").expect("file data");
    file.seek(SeekFrom::Start(0)).expect("rewind");
    let original = flags(&file);
    let mut input = ProxyStdio::new(&file, Interest::READABLE).expect("redirected input");
    let mut bytes = Vec::new();
    input.read_to_end(&mut bytes).await.expect("read file");
    assert_eq!(bytes, b"redirected input");
    assert!(input.readiness.is_none());
    drop(input);
    assert_eq!(flags(&file), original);

    let null = File::options()
        .read(true)
        .write(true)
        .open("/dev/null")
        .expect("null device");
    let mut input = ProxyStdio::new(&null, Interest::READABLE).expect("null input");
    assert_eq!(input.read(&mut [0; 1]).await.expect("null EOF"), 0);
    assert!(input.readiness.is_none());
    drop(input);
    let mut output = ProxyStdio::new(&null, Interest::WRITABLE).expect("null output");
    output.write_all(&bytes).await.expect("null write");
    output.flush().await.expect("unbuffered output");
    assert!(output.readiness.is_none());
}
