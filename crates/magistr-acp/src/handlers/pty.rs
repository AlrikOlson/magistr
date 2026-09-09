//! A pseudo-terminal pair for the agent's commands (Unix only): the
//! child gets the slave as its stdout and stderr so it sees a tty —
//! colours, progress bars, its own width — and the master is what the
//! transport reads and streams to the cockpit. Magistr's gate runner
//! spawns the quality gates on the same pair, so one pty serves every
//! command a cockpit watches.

use std::fs::File;
use std::os::fd::OwnedFd;
use std::process::Stdio;

use rustix::fs::{Mode, OFlags};
use rustix::io::{FdFlags, fcntl_dupfd_cloexec, fcntl_setfd};
use rustix::pty::{OpenptFlags, grantpt, openpt, ptsname, unlockpt};
use rustix::termios::{Winsize, tcsetwinsize};

/// One master/slave pair, sized, the master non-blocking for an
/// `AsyncFd` and neither side inherited by any child but the one it is
/// handed to.
pub struct Pty {
    master: OwnedFd,
    slave: OwnedFd,
}

impl Pty {
    /// Open a pair `cols` × `rows`.
    ///
    /// # Errors
    /// The first syscall that refused: no free pty, or a slave that
    /// would not open.
    pub fn open(cols: u16, rows: u16) -> std::io::Result<Self> {
        let master = openpt(OpenptFlags::RDWR | OpenptFlags::NOCTTY)?;
        fcntl_setfd(&master, FdFlags::CLOEXEC)?;
        grantpt(&master)?;
        unlockpt(&master)?;
        let name = ptsname(&master, Vec::new())?;
        let slave = rustix::fs::open(
            name,
            OFlags::RDWR | OFlags::NOCTTY | OFlags::CLOEXEC,
            Mode::empty(),
        )?;
        tcsetwinsize(
            &slave,
            Winsize {
                ws_row: rows,
                ws_col: cols,
                ws_xpixel: 0,
                ws_ypixel: 0,
            },
        )?;
        rustix::fs::fcntl_setfl(&master, OFlags::NONBLOCK)?;
        Ok(Self { master, slave })
    }

    /// A handle on the slave for one of the child's standard streams.
    ///
    /// # Errors
    /// The slave could not be duplicated.
    pub fn stdio(&self) -> std::io::Result<Stdio> {
        let fd = fcntl_dupfd_cloexec(&self.slave, 0)?;
        Ok(Stdio::from(File::from(fd)))
    }

    /// The master to read, and the slave the parent keeps open: macOS
    /// discards a slave's unread output when its last descriptor closes,
    /// so this one lives until the reader has drained after the exit.
    #[must_use]
    pub fn into_parts(self) -> (OwnedFd, OwnedFd) {
        (self.master, self.slave)
    }
}

#[cfg(test)]
#[path = "pty.tests.rs"]
mod tests;
