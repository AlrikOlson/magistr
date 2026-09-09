//! One thing a scripted terminal does.

use magistr_cockpit::cockpit::Size;

/// A step of a [`Script`](super::Script), due at some time after the
/// first read.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Step {
    /// The terminal sends these bytes (in one read, when the buffer fits).
    Bytes(Vec<u8>),
    /// The window changes size; the read in flight returns as a timeout
    /// so the input thread's size poll sees it.
    Resize(Size),
    /// The input closes: every later read is end of file.
    Close,
}
