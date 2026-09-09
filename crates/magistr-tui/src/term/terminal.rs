//! What the runtime needs from a terminal.

use std::io;

use super::input::TerminalInput;
use super::restorer::Restorer;
use magistr_cockpit::cockpit::Size;

/// The output side of a terminal plus a way to obtain its input side.
///
/// The runtime owns the `Terminal` on its own task and writes every frame
/// through it; the input side is taken once with [`Terminal::input`] and
/// moved to the input thread, so a blocking timed read never holds up a
/// write. [`RealTerminal`](super::RealTerminal) is the process's tty;
/// [`FakeTerminal`](crate::harness::FakeTerminal) captures bytes for tests.
///
/// A terminal may also hold the process's stderr for the session (see
/// [`OwnedStderr`](super::OwnedStderr)): [`Terminal::replay_stray`] hands
/// it back once the alternate screen is gone, and [`Terminal::restorer`]
/// mints the one-shot the panic hook uses to do the same from anywhere.
pub trait Terminal {
    /// The reader the input thread drives.
    type Input: TerminalInput;

    /// Write every byte of `bytes` (a frame, the session sequences).
    ///
    /// # Errors
    ///
    /// The underlying write failed.
    fn write(&mut self, bytes: &[u8]) -> io::Result<()>;

    /// The terminal's size in cells.
    ///
    /// # Errors
    ///
    /// The size could not be read.
    fn size(&self) -> io::Result<Size>;

    /// Take the input side. Called once by the runtime; a second call is
    /// an error.
    ///
    /// # Errors
    ///
    /// The input side was already taken, or could not be opened.
    fn input(&mut self) -> io::Result<Self::Input>;

    /// A one-shot that writes the bytes it is given and puts the terminal
    /// back — raw mode off, stderr handed back and replayed — from any
    /// thread: what the panic hook holds for the session.
    ///
    /// # Errors
    ///
    /// The descriptors the restorer needs could not be duplicated.
    fn restorer(&self) -> io::Result<Restorer>;

    /// Hand stderr back, when the terminal held it, and write whatever
    /// landed on it meanwhile so it reads in the scrollback. The runtime
    /// calls it right after the leave sequence; a terminal that never
    /// held stderr does nothing.
    ///
    /// # Errors
    ///
    /// Stderr could not be handed back, or the replay could not be
    /// written.
    fn replay_stray(&mut self) -> io::Result<()>;
}
