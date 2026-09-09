//! The scope chips of ⌘⇧F — a single select over the book's kinds.

use super::search_kind::SearchKind;

/// Which of the book's kinds a search reads: everything, or one kind
/// alone. The chips are a *select*, not actions:
/// choosing one narrows the same query rather than running a new one.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum SearchScope {
    /// Every kind, the strongest few of each.
    #[default]
    Everything,
    /// Chapters only.
    Reports,
    /// Receipts only.
    Receipts,
    /// Trace steps only.
    Trace,
    /// Sessions only.
    Sessions,
}

impl SearchScope {
    /// Every scope, in the order the chips sit.
    pub const ALL: [Self; 5] = [
        Self::Everything,
        Self::Reports,
        Self::Receipts,
        Self::Trace,
        Self::Sessions,
    ];

    /// The chip's word.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Everything => "everything",
            Self::Reports => "reports",
            Self::Receipts => "receipts",
            Self::Trace => "trace",
            Self::Sessions => "sessions",
        }
    }

    /// The one kind this scope reads; `None` for everything.
    #[must_use]
    pub const fn kind(self) -> Option<SearchKind> {
        match self {
            Self::Everything => None,
            Self::Reports => Some(SearchKind::Report),
            Self::Receipts => Some(SearchKind::Receipt),
            Self::Trace => Some(SearchKind::Trace),
            Self::Sessions => Some(SearchKind::Session),
        }
    }

    /// The chip to the right, wrapping — what ⇥ walks.
    #[must_use]
    pub const fn next(self) -> Self {
        match self {
            Self::Everything => Self::Reports,
            Self::Reports => Self::Receipts,
            Self::Receipts => Self::Trace,
            Self::Trace => Self::Sessions,
            Self::Sessions => Self::Everything,
        }
    }

    /// The chip to the left, wrapping — what ⇧⇥ walks.
    #[must_use]
    pub const fn prev(self) -> Self {
        match self {
            Self::Everything => Self::Sessions,
            Self::Reports => Self::Everything,
            Self::Receipts => Self::Reports,
            Self::Trace => Self::Receipts,
            Self::Sessions => Self::Trace,
        }
    }

    /// The scope at chip `index`, when there is one.
    #[must_use]
    pub fn at(index: usize) -> Option<Self> {
        Self::ALL.get(index).copied()
    }
}

#[cfg(test)]
#[path = "search_scope.tests.rs"]
mod tests;
