//! Raw mode for as long as the guard lives.

use std::io;
use std::os::fd::{AsFd, OwnedFd};

use rustix::termios::{OptionalActions, Termios, tcgetattr, tcsetattr};

use super::termios_restore::TermiosRestore;

/// A tty in raw mode: no echo, no line editing, no signal keys, input a
/// byte at a time. Dropping the guard — normally, or while unwinding from
/// a panic in the event loop — puts the saved termios back, so a crashed
/// cockpit never leaves the shell in raw mode.
///
/// The guard owns its own duplicate of the descriptor, so it restores the
/// terminal whatever the owner of the original does with it.
#[derive(Debug)]
pub struct RawMode {
    fd: OwnedFd,
    saved: Termios,
}

impl RawMode {
    /// Save `tty`'s termios and switch it to raw mode.
    ///
    /// # Errors
    ///
    /// `tty` is not a terminal, or its attributes could not be read or set.
    pub fn enter(tty: impl AsFd) -> io::Result<Self> {
        let fd = rustix::io::dup(tty)?;
        let saved = tcgetattr(&fd)?;
        let mut raw = saved.clone();
        raw.make_raw();
        tcsetattr(&fd, OptionalActions::Flush, &raw)?;
        Ok(Self { fd, saved })
    }

    /// The descriptor the guard restores on drop.
    #[must_use]
    pub fn fd(&self) -> std::os::fd::BorrowedFd<'_> {
        self.fd.as_fd()
    }

    /// A restore of the saved termios on a fresh duplicate of the
    /// descriptor, usable from any thread — what the panic hook holds.
    ///
    /// # Errors
    ///
    /// The descriptor could not be duplicated.
    pub fn restorer(&self) -> io::Result<TermiosRestore> {
        let fd = rustix::io::dup(&self.fd)?;
        Ok(TermiosRestore::new(fd, self.saved.clone()))
    }

    /// Whether `tty` is in raw mode as far as echo goes — the test's view
    /// of what [`RawMode::enter`] did and `Drop` undid.
    ///
    /// # Errors
    ///
    /// `tty` is not a terminal.
    pub fn is_raw(tty: impl AsFd) -> io::Result<bool> {
        let termios = tcgetattr(tty)?;
        Ok(!termios
            .local_modes
            .contains(rustix::termios::LocalModes::ECHO))
    }
}

impl Drop for RawMode {
    fn drop(&mut self) {
        // Nothing sensible to do with a failure here: the process is on
        // its way out of the session, and the descriptor may be gone.
        let _ = tcsetattr(&self.fd, OptionalActions::Now, &self.saved);
    }
}
