//! A mouse button.

/// The three primary pointer buttons; wheel actions are
/// reported as [`MouseKind`](super::MouseKind) variants instead.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Button {
    /// The primary button.
    Left,
    /// The wheel button.
    Middle,
    /// The secondary button.
    Right,
}
