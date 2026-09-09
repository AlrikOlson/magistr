//! One source line of a turn.

use super::line_kind::LineKind;
use crate::agent::ToolRecord;

/// A source line: its kind, its text (no newline), where its rows start
/// in the turn's row list, and — for a line one tool call's content
/// produced — which block of which call it came from.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Line {
    pub(super) kind: LineKind,
    pub(super) text: String,
    pub(super) row_start: usize,
    /// The tool call this line came from and the position of the block
    /// inside that call's content — how a call re-sending the same
    /// content finds the line it already pushed instead of pushing a
    /// second copy. `None` for every line the stream itself produced.
    pub(super) source: Option<(String, usize)>,
    /// What the tool call this line *is* said past its row — the body
    /// a card opens into. Beside the kind rather than inside it: the
    /// kind keys the wrap, and the record is data, never a row. Empty
    /// for every line that is not a tool call or an edit.
    pub(super) record: ToolRecord,
    /// Whether the program on this line's terminal reports its
    /// progress (OSC 9;4): the card's pane then carries a strip row
    /// under the head, ahead of the screen's rows.
    pub(super) strip: bool,
}

impl Line {
    /// The line's kind.
    #[must_use]
    pub const fn kind(&self) -> &LineKind {
        &self.kind
    }

    /// The record the line's tool call keeps — empty unless the line
    /// is a tool call or an edit that said something past its row.
    #[must_use]
    pub const fn record(&self) -> &ToolRecord {
        &self.record
    }

    /// The line's text.
    #[must_use]
    pub fn text(&self) -> &str {
        &self.text
    }

    /// Index of the line's first row in the turn's rows.
    #[must_use]
    pub const fn row_start(&self) -> usize {
        self.row_start
    }
}
