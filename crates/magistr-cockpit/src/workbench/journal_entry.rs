//! One journaled edit, as the workbench recorded it.

/// An entry of the workbench's write journal — the authoritative record
/// of one edit: which file, how many lines came and went, whether the
/// write parse-verified, and the unified diff the journal kept.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct JournalEntry {
    /// The journal's id for the edit — what an undo names.
    pub id: String,
    /// The edited file, workspace-relative.
    pub path: String,
    /// Lines added.
    pub added: u32,
    /// Lines removed.
    pub removed: u32,
    /// Whether the write parse-verified.
    pub parse_ok: bool,
    /// The unified diff body (`@@` hunk headers, `+`/`-`/` ` lines).
    pub diff: String,
}
