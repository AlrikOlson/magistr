//! Timed reads from a terminal descriptor.

use std::io;
use std::os::fd::{AsFd, BorrowedFd, OwnedFd};
use std::time::Duration;

use rustix::event::Timespec;
use rustix::event::{PollFd, PollFlags, poll};
use rustix::io::Errno;

use super::input::TerminalInput;
use super::winsize::winsize;
use magistr_cockpit::cockpit::Size;

/// The read side of a [`RealTerminal`](super::RealTerminal): `poll` for
/// readability with a timeout, then one `read`.
#[derive(Debug)]
pub struct RealInput {
    fd: OwnedFd,
}

impl RealInput {
    /// Reads from `fd`.
    #[must_use]
    pub const fn new(fd: OwnedFd) -> Self {
        Self { fd }
    }

    /// The descriptor being read.
    #[must_use]
    pub fn fd(&self) -> BorrowedFd<'_> {
        self.fd.as_fd()
    }

    /// Wait up to `timeout` for input, then read what is there. `None` on
    /// a timeout (or a signal interrupting the wait), `Some(0)` once the
    /// descriptor is at end of file.
    ///
    /// # Errors
    ///
    /// The poll or the read failed.
    pub fn read_timeout(&mut self, buf: &mut [u8], timeout: Duration) -> io::Result<Option<usize>> {
        let mut fds = [PollFd::new(&self.fd, PollFlags::IN)];
        let timeout = timespec(timeout)?;
        match poll(&mut fds, Some(&timeout)) {
            Ok(0) | Err(Errno::INTR) => return Ok(None),
            Ok(_) => {}
            Err(e) => return Err(e.into()),
        }
        match rustix::io::read(&self.fd, buf) {
            Ok(n) => Ok(Some(n)),
            Err(Errno::INTR | Errno::AGAIN) => Ok(None),
            // A pty whose master went away answers EIO on Linux; that is
            // the end of the input as much as a zero read is.
            Err(Errno::IO) => Ok(Some(0)),
            Err(e) => Err(e.into()),
        }
    }
}

impl TerminalInput for RealInput {
    fn read(&mut self, buf: &mut [u8], timeout: Duration) -> io::Result<Option<usize>> {
        self.read_timeout(buf, timeout)
    }

    fn size(&self) -> io::Result<Size> {
        winsize(&self.fd)
    }
}

fn timespec(duration: Duration) -> io::Result<Timespec> {
    let invalid = |_| io::Error::new(io::ErrorKind::InvalidInput, "timeout out of range");
    Ok(Timespec {
        tv_sec: i64::try_from(duration.as_secs()).map_err(invalid)?,
        tv_nsec: duration.subsec_nanos().into(),
    })
}
