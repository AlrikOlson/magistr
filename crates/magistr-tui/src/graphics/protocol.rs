//! Which pixel protocol the cockpit will speak.

use super::passthrough::Passthrough;

/// A graphics protocol the probe found the terminal to support.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Protocol {
    /// The kitty graphics protocol, commands wrapped as `Passthrough` says.
    Kitty(Passthrough),
    /// Sixel (DA1 parameter 4), only outside tmux.
    Sixel,
}
