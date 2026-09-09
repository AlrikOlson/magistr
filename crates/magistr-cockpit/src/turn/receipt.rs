//! What a completed tool call left on the session's ledger.

/// The receipt a completed tool call earns: its ordinal on the
/// session's one global counter (the same counter the rail's
/// `N receipts after cp-N` is taken from, so the two readings cannot
/// disagree), the lines its edit added and removed, and the newest
/// checkpoint whose rollback reach covers it.
///
/// Derived at the completion fold and read only by the native card's
/// head and trailer — the cell renderer never shows one, which is what
/// keeps every terminal golden unmoved.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Receipt {
    /// Lines the edit added — zero for a call with no diff.
    pub added: u32,
    /// Lines the edit removed — zero for a call with no diff.
    pub removed: u32,
    /// The receipt's number on the session's global counter.
    pub ordinal: u32,
    /// The checkpoint this receipt is inside the reach of — the spine's
    /// head when the call completed; `None` when nothing was
    /// checkpointed, and the trailer then names no rollback anchor.
    pub reach: Option<String>,
    /// Whether the reader ended the program with the held kill:
    /// the head
    /// and the receipt line then say `killed by you`, so a run the
    /// human stopped is never read as one that failed on its own.
    pub killed: bool,
}
