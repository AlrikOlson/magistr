//! Cancellable stdio for the process-scoped owner proxy.
//!
//! Tokio's stdio adapters start uncancellable blocking operations. An idle
//! client or full output pipe could keep the runtime alive after its peer
//! disconnects. Poll owned duplicates instead, restoring their shared flags
//! when the proxy ends or readiness registration fails.

use std::fs::File;
use std::io::{self, Read, Write};
use std::os::fd::{AsFd, AsRawFd, RawFd};
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll, ready};

use tokio::io::unix::AsyncFd;
use tokio::io::{AsyncRead, AsyncWrite, Interest, ReadBuf};

pub(super) struct ProxyStdio {
    file: Arc<NonblockingFile>,
    readiness: Option<AsyncFd<Arc<NonblockingFile>>>,
    interest: Interest,
}

impl ProxyStdio {
    pub(super) fn stdin() -> io::Result<Self> {
        Self::new(&io::stdin(), Interest::READABLE)
    }

    pub(super) fn stdout() -> io::Result<Self> {
        Self::new(&io::stdout(), Interest::WRITABLE)
    }

    fn new(descriptor: &impl AsFd, interest: Interest) -> io::Result<Self> {
        let file = File::from(descriptor.as_fd().try_clone_to_owned()?);
        Ok(Self {
            file: Arc::new(NonblockingFile::new(file)?),
            readiness: None,
            interest,
        })
    }

    fn poll_io<T>(
        &mut self,
        context: &mut Context<'_>,
        mut operation: impl FnMut(&File) -> io::Result<T>,
    ) -> Poll<io::Result<T>> {
        loop {
            let result = if let Some(descriptor) = &self.readiness {
                let mut readiness = if self.interest.is_readable() {
                    ready!(descriptor.poll_read_ready(context))?
                } else {
                    ready!(descriptor.poll_write_ready(context))?
                };
                match readiness.try_io(|descriptor| operation(&descriptor.get_ref().file)) {
                    Ok(result) => result,
                    Err(_) => continue,
                }
            } else {
                operation(&self.file.file)
            };
            match result {
                Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                    // Register only when I/O needs to wait. Regular files and
                    // /dev/null never do, and epoll cannot register them.
                    self.readiness = Some(AsyncFd::with_interest(
                        Arc::clone(&self.file),
                        self.interest,
                    )?);
                }
                result => return Poll::Ready(result),
            }
        }
    }
}

impl AsyncRead for ProxyStdio {
    fn poll_read(
        mut self: Pin<&mut Self>,
        context: &mut Context<'_>,
        buffer: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        if buffer.remaining() == 0 {
            return Poll::Ready(Ok(()));
        }
        self.poll_io(context, |mut file| {
            let count = file.read(buffer.initialize_unfilled())?;
            buffer.advance(count);
            Ok(())
        })
    }
}

impl AsyncWrite for ProxyStdio {
    fn poll_write(
        mut self: Pin<&mut Self>,
        context: &mut Context<'_>,
        bytes: &[u8],
    ) -> Poll<io::Result<usize>> {
        if bytes.is_empty() {
            return Poll::Ready(Ok(0));
        }
        self.poll_io(context, |mut file| file.write(bytes))
    }

    fn poll_flush(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<io::Result<()>> {
        // File writes are unbuffered; there is no userspace buffer to flush.
        Poll::Ready(Ok(()))
    }

    fn poll_shutdown(self: Pin<&mut Self>, context: &mut Context<'_>) -> Poll<io::Result<()>> {
        self.poll_flush(context)
    }
}

struct NonblockingFile {
    file: File,
    original_flags: libc::c_int,
}

impl NonblockingFile {
    fn new(file: File) -> io::Result<Self> {
        // SAFETY: file owns the descriptor throughout this call.
        let original_flags = unsafe { libc::fcntl(file.as_raw_fd(), libc::F_GETFL) };
        if original_flags == -1 {
            return Err(io::Error::last_os_error());
        }
        let input = Self {
            file,
            original_flags,
        };
        // Establish the restoring owner before changing flags: registration
        // failure must restore them just as normal proxy shutdown does.
        // SAFETY: input owns the descriptor and F_SETFL takes integer flags.
        if unsafe {
            libc::fcntl(
                input.as_raw_fd(),
                libc::F_SETFL,
                original_flags | libc::O_NONBLOCK,
            )
        } == -1
        {
            return Err(io::Error::last_os_error());
        }
        Ok(input)
    }
}

impl AsRawFd for NonblockingFile {
    fn as_raw_fd(&self) -> RawFd {
        self.file.as_raw_fd()
    }
}

impl Drop for NonblockingFile {
    fn drop(&mut self) {
        // SAFETY: the file still owns this descriptor; dup shares status flags
        // with the original, so restore them before closing our duplicate.
        if unsafe { libc::fcntl(self.as_raw_fd(), libc::F_SETFL, self.original_flags) } == -1 {
            tracing::warn!(error = %io::Error::last_os_error(), "could not restore proxy stdio flags");
        }
    }
}

#[cfg(test)]
#[path = "owner_stdio.tests.rs"]
mod tests;
