//! Putting a terminal back from anywhere.

/// A one-shot that writes the bytes it is given (the leave sequence) to
/// the terminal and then puts the terminal back — termios, stderr — from
/// whatever thread calls it. [`Terminal::restorer`](super::Terminal::restorer)
/// mints one; the [`PanicHook`](super::PanicHook) holds it for the life
/// of the session.
pub type Restorer = Box<dyn FnOnce(&[u8]) + Send>;
