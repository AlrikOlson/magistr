//! Where in the turn's content a window is held.

/// A place in the turn's *content* rather than in its rows: the source
/// line under a row and how many rows into that line's wrap the row
/// sits. Rows move whenever the turn re-wraps — a card opening above the
/// window pushes every row after it down by its body — but a line keeps
/// its index for the life of the turn, so a window held to an anchor
/// resolves back to the row its content moved to
/// ([`Turn::row_of`](super::Turn::row_of)) and the reader keeps their
/// place. The row offset is clamped to the line's wrap on the way back,
/// so a window that was inside a card's body when the card folded lands
/// on the card's head.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Anchor {
    line: usize,
    row: usize,
}

impl Anchor {
    /// The anchor `row` rows into line `line`'s wrap.
    #[must_use]
    pub const fn new(line: usize, row: usize) -> Self {
        Self { line, row }
    }

    /// The source line's index in the turn.
    #[must_use]
    pub const fn line(&self) -> usize {
        self.line
    }

    /// How many rows into the line's wrap the anchor sits.
    #[must_use]
    pub const fn row(&self) -> usize {
        self.row
    }
}

#[cfg(test)]
#[path = "anchor.tests.rs"]
mod tests;
