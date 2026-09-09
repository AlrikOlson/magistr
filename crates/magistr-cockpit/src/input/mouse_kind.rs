//! What the mouse did.

use super::button::Button;

/// The kind of a mouse report.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum MouseKind {
    /// A button went down.
    Down(Button),
    /// A button came up.
    Up(Button),
    /// The pointer moved with a button held.
    Drag(Button),
    /// The pointer moved with no button held.
    Move,
    /// The wheel rolled away from the user.
    ScrollUp,
    /// The wheel rolled toward the user.
    ScrollDown,
    /// Horizontal wheel, left.
    ScrollLeft,
    /// Horizontal wheel, right.
    ScrollRight,
}
