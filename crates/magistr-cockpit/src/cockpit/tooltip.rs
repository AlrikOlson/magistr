//! The card the pointer earns by resting.

/// A hover card's text and anchor cell.
///
/// [`Model::tooltip`](super::Model::tooltip) derives the card, and `view`
/// draws it on the overlay layer. It does not affect layout and is clipped
/// to the screen rather than its parent, so a footer tooltip remains visible.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Tooltip {
    /// What the widget is called.
    pub text: &'static str,
    /// The pointer's column.
    pub col: u16,
    /// The pointer's row.
    pub row: u16,
}

impl Tooltip {
    /// Cells of padding either side of the text inside the card.
    pub const PAD: u16 = 1;

    /// The card's width in cells, text and padding.
    #[must_use]
    pub fn cols(&self) -> u16 {
        let text = u16::try_from(self.text.chars().count()).unwrap_or(u16::MAX);
        text.saturating_add(Self::PAD * 2)
    }

    /// The card's top-left cell on a `cols` x `rows` screen.
    ///
    /// Below and right of the pointer, which is where a card belongs
    /// when the pointer is an arrow drawn up and to the left — and
    /// flipped to the other side of the pointer at either far edge
    /// rather than clamped, because a card clamped against the edge sits
    /// *under* the pointer and hides the thing it is describing.
    #[must_use]
    pub fn place(&self, cols: u16, rows: u16) -> (u16, u16) {
        let width = self.cols();
        let col = if self.col.saturating_add(1 + width) <= cols {
            self.col + 1
        } else {
            self.col.saturating_sub(width)
        };
        let row = if self.row + 2 <= rows {
            self.row + 1
        } else {
            self.row.saturating_sub(1)
        };
        (col.min(cols.saturating_sub(width)), row)
    }
}

#[cfg(test)]
#[path = "tooltip.tests.rs"]
mod tests;
