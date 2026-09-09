//! One run of a diff row's text, by whether it is what changed.

/// A stretch of one [`HunkRow`](super::HunkRow)'s text, marked with
/// whether the word diff found it changed.
///
/// The row's whole text is its spans in order, so a consumer that does
/// not care about the emphasis can simply concatenate them and a
/// consumer that does needs no second pass over the line. `changed` is
/// false for every span of a context row and for the parts of a
/// replaced row the other side also has — the point of a word-level
/// highlight is that it says which *part* of the line moved, which a
/// row-level `+`/`-` cannot.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HunkSpan {
    /// The text of the run.
    pub text: String,
    /// Whether this run is what the word diff found different.
    pub changed: bool,
}

impl HunkSpan {
    /// A run of `text` that the word diff found changed.
    #[must_use]
    pub fn changed(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            changed: true,
        }
    }

    /// A run of `text` both sides share.
    #[must_use]
    pub fn same(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            changed: false,
        }
    }
}
