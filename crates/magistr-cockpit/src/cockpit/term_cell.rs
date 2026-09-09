//! One cell of a program's terminal screen.

use crate::render::{Attrs, Color};

/// A cell of a [`Term`](super::term::Term)'s screen as the program
/// painted it: its grapheme, its colours as the program named them —
/// the terminal's default, an ANSI or 256-colour index, or truecolor —
/// and its attributes. The painter resolves the colours through the
/// theme; here they are the program's own.
///
/// A wide grapheme's second column is a cell with no text at all.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TermCell {
    /// The grapheme, empty for the continuation of a wide one.
    pub text: String,
    /// The foreground as the program named it.
    pub fg: Color,
    /// The background as the program named it.
    pub bg: Color,
    /// The attributes the program set.
    pub attrs: Attrs,
}

impl TermCell {
    /// Whether the cell shows nothing of the program's — no grapheme,
    /// or the tail of a wide one.
    #[must_use]
    pub fn is_blank(&self) -> bool {
        self.text.is_empty()
    }

    /// Whether this cell paints like `other`: same colours, same
    /// attributes — what lets a row be sent as runs.
    #[must_use]
    pub fn same_pen(&self, other: &Self) -> bool {
        self.fg == other.fg && self.bg == other.bg && self.attrs == other.attrs
    }
}
