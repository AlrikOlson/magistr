//! Putting a tty's termios back on demand.

use std::io;
use std::os::fd::OwnedFd;

use rustix::termios::{OptionalActions, Termios, tcsetattr};

/// The termios a [`RawMode`](super::RawMode) guard saved, on a duplicate
/// of its descriptor, restorable from any thread at any time — what the
/// panic hook holds.
///
/// Unlike the guard it has no `Drop`: handing it around never cooks a
/// terminal that is still in use.
#[derive(Debug)]
pub struct TermiosRestore {
    fd: OwnedFd,
    saved: Termios,
}

impl TermiosRestore {
    /// A restore for `fd` back to `saved`.
    #[must_use]
    pub const fn new(fd: OwnedFd, saved: Termios) -> Self {
        Self { fd, saved }
    }

    /// Put the saved termios back now.
    ///
    /// # Errors
    ///
    /// The attributes could not be set (the descriptor may be gone).
    pub fn restore(&self) -> io::Result<()> {
        tcsetattr(&self.fd, OptionalActions::Now, &self.saved)?;
        Ok(())
    }
}
