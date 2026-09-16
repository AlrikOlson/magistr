//! One ⌘⇧F result, as `update` computes it and a composition renders it.

use super::search_kind::SearchKind;

/// One hit: what it is, the line it shows with the matched runs marked,
/// and when it happened. What confirming it opens stays crate-side —
/// a surface renders the row; only `update` acts on it.
#[derive(Clone, Debug, PartialEq)]
pub struct SearchHit {
    /// The book's kind the hit belongs to.
    pub kind: SearchKind,
    /// The title over the line — a chapter's — when the kind has one.
    pub title: Option<String>,
    /// The line: a windowed snippet with its `…`, or the whole identity
    /// of a receipt or a session.
    pub line: String,
    /// Byte ranges into `line` that matched the query, merged and in
    /// order — what a composition sets bold in the accent.
    pub marks: Vec<(usize, usize)>,
    /// When it happened, as the corpus stamps it (`Aug 29`), when it
    /// carries a stamp.
    pub when: Option<String>,
    /// What ↵ opens.
    pub(crate) go: SearchGo,
}

/// What confirming a hit does: open the thing at the match, at the
/// altitude it lives at.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum SearchGo {
    /// Open the chunk through the one open-chunk fold.
    Chunk(String),
    /// The Trace with the step expanded.
    Step(u32),
    /// The Trace, where the ship timeline is.
    Receipt(String),
    /// The launcher's session row.
    Session(usize),
}
