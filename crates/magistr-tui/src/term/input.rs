//! The input side of a terminal: a read that gives up after a while.

use std::io;
use std::time::Duration;

use magistr_cockpit::cockpit::Size;

/// A byte source the input thread reads from with a timeout, plus the
/// window size it polls whenever a read comes back empty.
pub trait TerminalInput: Send + 'static {
    /// Read into `buf`, waiting at most `timeout` for the first byte.
    /// `Ok(None)` is a timeout, `Ok(Some(0))` means the source is closed
    /// (end of file, the other end hung up), `Ok(Some(n))` is `n` bytes.
    ///
    /// # Errors
    ///
    /// The underlying poll or read failed.
    fn read(&mut self, buf: &mut [u8], timeout: Duration) -> io::Result<Option<usize>>;

    /// The terminal's size in cells right now.
    ///
    /// # Errors
    ///
    /// The size could not be read.
    fn size(&self) -> io::Result<Size>;
}
