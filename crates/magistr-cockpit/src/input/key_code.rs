//! Which key.

/// A key, independent of how the terminal encoded it.
///
/// Letters typed with shift arrive as their uppercase [`KeyCode::Char`]
/// with `SHIFT` set, whichever encoding the terminal used.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum KeyCode {
    /// A printable character (keypad digits and operators included).
    Char(char),
    /// Enter / Return (keypad Enter included).
    Enter,
    /// Tab.
    Tab,
    /// Backspace (`0x7f` or `0x08`).
    Backspace,
    /// Escape.
    Esc,
    /// Cursor up.
    Up,
    /// Cursor down.
    Down,
    /// Cursor left.
    Left,
    /// Cursor right.
    Right,
    /// Home.
    Home,
    /// End.
    End,
    /// Page up.
    PageUp,
    /// Page down.
    PageDown,
    /// Insert.
    Insert,
    /// Delete (forward).
    Delete,
    /// A function key, `F(1)` through `F(35)`.
    F(u8),
}
