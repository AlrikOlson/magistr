//! One line of a hunk: its side, its line numbers, its text in runs.

use super::hunk_span::HunkSpan;
use super::row_kind::RowKind;

/// A line of a unified diff as a viewer draws it: what kind of line it
/// is, the number it carries on each side, and its text split into the
/// runs the word diff found.
///
/// Both numbers are kept rather than one plus a side, because a gutter
/// shows *both* columns and a side-by-side layout reads one each — the
/// number a row wears is a fact about the row, not about the layout it
/// is being drawn in. A removed line has no number on the new side and
/// an added line none on the old, which is exactly the blank a gutter
/// prints.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HunkRow {
    /// What the line is, through [`diff_kind`](super::diff_kind) — so
    /// the viewer and the cell renderer's edit card cannot disagree
    /// about whether a line is added, removed, context or a conflict
    /// marker.
    pub kind: RowKind,
    /// The line's number in the old file, when it has one.
    pub old_no: Option<u32>,
    /// The line's number in the new file, when it has one.
    pub new_no: Option<u32>,
    /// The line's text (no marker, no newline), in word-diff runs.
    pub spans: Vec<HunkSpan>,
}

impl HunkRow {
    /// The line's whole text — its spans, concatenated.
    #[must_use]
    pub fn text(&self) -> String {
        self.spans.iter().map(|span| span.text.as_str()).collect()
    }

    /// The marker column: `-`, `+` or a space, as a unified diff writes
    /// it.
    #[must_use]
    pub const fn marker(&self) -> &'static str {
        match self.kind {
            RowKind::DiffRemove => "-",
            RowKind::DiffAdd => "+",
            _ => " ",
        }
    }

    /// The gutter: the two line numbers right-aligned in `width`
    /// columns each, a blank where a side has none.
    #[must_use]
    pub fn gutter(&self, width: usize) -> String {
        format!(
            "{} {}",
            Self::number(self.old_no, width),
            Self::number(self.new_no, width)
        )
    }

    /// One side of the gutter for a side-by-side column: the old number
    /// for a removed line, the new one otherwise.
    #[must_use]
    pub fn side_gutter(&self, width: usize, old_side: bool) -> String {
        let number = if old_side { self.old_no } else { self.new_no };
        Self::number(number, width)
    }

    fn number(value: Option<u32>, width: usize) -> String {
        match value {
            Some(n) => format!("{n:>width$}"),
            None => " ".repeat(width),
        }
    }
}
