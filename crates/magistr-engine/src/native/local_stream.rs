//! The connection an owner endpoint hands an attached session.
//!
//! One name for each platform's stream — a Unix socket, or either end of a
//! Windows pipe — so the handshake, the MCP acceptor and the attach path
//! are written once. The definitions change together because every one of
//! them must stay a plain byte stream to the code above it.

/// One end of an owner connection: a Unix domain socket.
#[cfg(unix)]
pub type LocalStream = tokio::net::UnixStream;

/// One end of an owner connection: the pipe instance an owner accepted,
/// or the client end a process attached through.
#[cfg(windows)]
#[derive(Debug)]
pub enum LocalStream {
    /// The owner's side of an accepted connection.
    Server(tokio::net::windows::named_pipe::NamedPipeServer),
    /// An attached client's side.
    Client(tokio::net::windows::named_pipe::NamedPipeClient),
}

#[cfg(windows)]
mod pipe_io {
    use std::{
        io,
        pin::Pin,
        task::{Context, Poll},
    };

    use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};

    use super::LocalStream;

    impl AsyncRead for LocalStream {
        fn poll_read(
            self: Pin<&mut Self>,
            cx: &mut Context<'_>,
            buf: &mut ReadBuf<'_>,
        ) -> Poll<io::Result<()>> {
            match self.get_mut() {
                Self::Server(pipe) => Pin::new(pipe).poll_read(cx, buf),
                Self::Client(pipe) => Pin::new(pipe).poll_read(cx, buf),
            }
        }
    }

    impl AsyncWrite for LocalStream {
        fn poll_write(
            self: Pin<&mut Self>,
            cx: &mut Context<'_>,
            buf: &[u8],
        ) -> Poll<io::Result<usize>> {
            match self.get_mut() {
                Self::Server(pipe) => Pin::new(pipe).poll_write(cx, buf),
                Self::Client(pipe) => Pin::new(pipe).poll_write(cx, buf),
            }
        }

        fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
            match self.get_mut() {
                Self::Server(pipe) => Pin::new(pipe).poll_flush(cx),
                Self::Client(pipe) => Pin::new(pipe).poll_flush(cx),
            }
        }

        fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
            match self.get_mut() {
                Self::Server(pipe) => Pin::new(pipe).poll_shutdown(cx),
                Self::Client(pipe) => Pin::new(pipe).poll_shutdown(cx),
            }
        }
    }
}

#[cfg(test)]
#[path = "local_stream.tests.rs"]
mod tests;
