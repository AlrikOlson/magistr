//! How wide a pane has to be before a diff can afford each reading.

/// The three readings of a diff, widest first.
///
/// Which one a review draws is a property of the **pane**, never a mode
/// the reader has to find: the same diff in a narrow column and in a
/// wide one wants two different arrangements, and a viewer that made
/// the reader choose would be asking them a question the layout already
/// knows the answer to. That is also what keeps the vocabulary honest —
/// side-by-side is not a feature that is on, it is what a pane with
/// room for two columns shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DiffLayout {
    /// Old and new in two columns, each with its own line number.
    Split,
    /// One column, line numbers on both sides in the gutter.
    Gutter,
    /// One column, marker only — a pane too narrow to spend columns on
    /// numbers, which is the one case where the text itself is what has
    /// to survive.
    Compact,
}

impl DiffLayout {
    /// Columns between the two halves of a split reading.
    pub const SPLIT_GAP: usize = 3;

    /// The widest reading `cols` can hold for a diff whose line numbers
    /// take `gutter` columns and whose widest line is `widest`
    /// characters.
    ///
    /// Each tier is asked for exactly the room it draws in, so nothing
    /// is ever cut off by a rule that guessed: a split needs both
    /// columns whole, a gutter needs both numbers plus the marker, and
    /// the compact reading is what is left when neither fits.
    #[must_use]
    pub const fn for_width(cols: usize, gutter: usize, widest: usize) -> Self {
        let column = gutter + 1 + widest;
        if 2 * column + Self::SPLIT_GAP <= cols {
            return Self::Split;
        }
        if 2 * gutter + 2 + widest <= cols {
            return Self::Gutter;
        }
        Self::Compact
    }

    /// Whether this reading puts old and new in two columns.
    #[must_use]
    pub const fn is_split(self) -> bool {
        matches!(self, Self::Split)
    }

    /// Whether this reading prints line numbers at all.
    #[must_use]
    pub const fn has_gutter(self) -> bool {
        !matches!(self, Self::Compact)
    }
}

#[cfg(test)]
#[path = "diff_layout.tests.rs"]
mod tests;
