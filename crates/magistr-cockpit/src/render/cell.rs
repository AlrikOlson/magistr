//! One grid cell: a grapheme cluster and its style.

use compact_str::CompactString;

use super::style::Style;
use super::width::width;

/// A single cell of the grid.
///
/// A grapheme wider than one column occupies a *head* cell holding the
/// symbol followed by *continuation* cells with an empty symbol; the
/// [`Buffer`](super::Buffer) keeps that invariant on every write.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Cell {
    /// The grapheme cluster shown in this cell; empty for a continuation.
    pub symbol: CompactString,
    /// How the cell is painted.
    pub style: Style,
}

impl Cell {
    /// A cell showing `symbol` in `style`.
    #[must_use]
    pub fn new(symbol: &str, style: Style) -> Self {
        Self {
            symbol: CompactString::new(symbol),
            style,
        }
    }

    /// A space in the default style.
    #[must_use]
    pub fn blank() -> Self {
        Self::new(" ", Style::default())
    }

    /// The trailing column(s) of a wide grapheme.
    #[must_use]
    pub fn continuation() -> Self {
        Self::new("", Style::default())
    }

    /// True for the trailing column(s) of a wide grapheme.
    #[must_use]
    pub fn is_continuation(&self) -> bool {
        self.symbol.is_empty()
    }

    /// Columns this cell's symbol occupies (0 for a continuation).
    #[must_use]
    pub fn width(&self) -> u16 {
        width(&self.symbol)
    }
}

impl Default for Cell {
    fn default() -> Self {
        Self::blank()
    }
}

#[cfg(test)]
#[path = "cell.tests.rs"]
mod tests;
