//! Press, repeat or release.

/// Whether a key was pressed, repeated while held, or released.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum KeyKind {
    /// The key went down.
    #[default]
    Press,
    /// The key is held and auto-repeating.
    Repeat,
    /// The key came up.
    Release,
}
