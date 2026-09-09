//! The process terminal where there is no tty the cockpit can drive.

use std::io;

use super::restorer::Restorer;
use super::terminal::Terminal;
use super::unsupported_input::RealInput;
use magistr_cockpit::cockpit::Size;

/// The cockpit's terminal on a platform without a Unix tty (Windows).
///
/// [`RealTerminal::stdio`] is the one thing that can be called, and it
/// fails with [`io::ErrorKind::Unsupported`] — on the normal console,
/// before the session sequences, raw mode or stderr are touched — so
/// every cockpit entry point reports the missing backend and exits.
/// The type is uninhabited: nothing ever writes a frame through it.
///
/// The Unix implementation (termios raw mode, `poll` on stdin, the
/// window size from `TIOCGWINSZ`) is `term/real.rs`; a Windows console
/// backend (console modes, a waited `ReadFile`) would replace this.
#[derive(Debug)]
pub enum RealTerminal {}

impl RealTerminal {
    /// Refuse to take the terminal over: there is no backend for it here.
    ///
    /// # Errors
    ///
    /// Always, with [`io::ErrorKind::Unsupported`].
    pub fn stdio() -> io::Result<Self> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "the cockpit needs a Unix terminal on stdin and stdout; \
             this platform has no terminal backend yet",
        ))
    }
}

impl Terminal for RealTerminal {
    type Input = RealInput;

    fn write(&mut self, _bytes: &[u8]) -> io::Result<()> {
        match *self {}
    }

    fn size(&self) -> io::Result<Size> {
        match *self {}
    }

    fn input(&mut self) -> io::Result<RealInput> {
        match *self {}
    }

    fn restorer(&self) -> io::Result<Restorer> {
        match *self {}
    }

    fn replay_stray(&mut self) -> io::Result<()> {
        match *self {}
    }
}

#[cfg(test)]
#[path = "unsupported.tests.rs"]
mod tests;
