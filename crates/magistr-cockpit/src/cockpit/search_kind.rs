//! The kinds a search hit can be — the book's own.

/// What a ⌘⇧F hit *is*: one of the four kinds the book keeps, in the
/// order the results list groups them. A hit says
/// what it is before it says what it found — the group cap is the
/// kind's word, never a guess from the text.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SearchKind {
    /// A chapter — a shipped chunk, opened as its report.
    Report,
    /// A reasoning step off the think trace.
    Trace,
    /// A receipt — an action the ship ledger recorded.
    Receipt,
    /// A recent session as the launcher lists it.
    Session,
}

impl SearchKind {
    /// Every kind, in the order the results list groups them.
    pub const ALL: [Self; 4] = [Self::Report, Self::Trace, Self::Receipt, Self::Session];

    /// The word the group cap wears.
    #[must_use]
    pub const fn word(self) -> &'static str {
        match self {
            Self::Report => "reports",
            Self::Trace => "trace",
            Self::Receipt => "receipts",
            Self::Session => "sessions",
        }
    }
}
