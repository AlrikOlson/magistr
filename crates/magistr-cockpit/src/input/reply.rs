//! The terminal's answers to the cockpit's startup queries.

use crate::render::Color;

/// A reply to a query the session sent at startup.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Reply {
    /// OSC 11: the terminal's background colour, for dark/light detection.
    Background(Color),
    /// `CSI ? flags u`: the kitty keyboard protocol flags in effect —
    /// the terminal speaks the protocol.
    KeyboardFlags(u8),
    /// `CSI 6 ; height ; width t`: one cell's size in pixels (the answer
    /// to `CSI 16 t`), what a pixel widget scales by.
    CellPixels {
        /// Cell width in pixels.
        width: u16,
        /// Cell height in pixels.
        height: u16,
    },
    /// `CSI 4 ; height ; width t`: the text area's size in pixels (the
    /// answer to `CSI 14 t`), the fallback for cell size when divided by
    /// the size in cells.
    WindowPixels {
        /// Width in pixels.
        width: u16,
        /// Height in pixels.
        height: u16,
    },
}
