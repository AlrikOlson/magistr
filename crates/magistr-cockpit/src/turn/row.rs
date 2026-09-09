//! One row of the stage.

use super::row_kind::RowKind;
use super::span::Span;

/// A wrapped row: its kind, the text that fits the stage width, and the
/// inline spans styling parts of that text (empty means the whole row
/// takes the kind's pen).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Row {
    /// How the row is styled.
    pub kind: RowKind,
    /// The row's text, at most the wrap width in columns.
    pub text: String,
    /// Styled runs over `text`, ordered and non-overlapping.
    pub spans: Vec<Span>,
}

impl Row {
    /// A row of `kind` showing `text`, all of it in the kind's pen.
    #[must_use]
    pub fn new(kind: RowKind, text: impl Into<String>) -> Self {
        Self {
            kind,
            text: text.into(),
            spans: Vec::new(),
        }
    }

    /// A row of `kind` showing `text` with `spans` styling runs of it.
    #[must_use]
    pub fn styled(kind: RowKind, text: impl Into<String>, spans: Vec<Span>) -> Self {
        Self {
            kind,
            text: text.into(),
            spans,
        }
    }
}
