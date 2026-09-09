//! The input side of [`RealTerminal`](super::unsupported::RealTerminal)
//! where there is no tty: never handed out, so never read.

use std::io;
use std::time::Duration;

use super::input::TerminalInput;
use magistr_cockpit::cockpit::Size;

/// The reader a [`RealTerminal`](super::unsupported::RealTerminal) would
/// hand the input thread. The terminal is uninhabited, so this is too:
/// the input thread never runs on this platform.
#[derive(Debug)]
pub enum RealInput {}

impl TerminalInput for RealInput {
    fn read(&mut self, _buf: &mut [u8], _timeout: Duration) -> io::Result<Option<usize>> {
        match *self {}
    }

    fn size(&self) -> io::Result<Size> {
        match *self {}
    }
}
