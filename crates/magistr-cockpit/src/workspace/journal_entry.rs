//! One journaled edit, as the workspace recorded it.

/// An entry of the workspace's write journal — the authoritative record
/// of one edit: which file, how many lines came and went, whether the
/// write parse-verified, and the unified diff the journal kept.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct JournalEntry {
    /// The journal's id for the edit — what an undo names.
    pub id: String,
    /// Journal-assigned Unix milliseconds; absent only in legacy records.
    pub at_ms: Option<i64>,
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

impl JournalEntry {
    /// A file row's identity; a multi-file operation shares its undo id.
    #[must_use]
    pub fn identity(&self) -> String {
        format!("{}\0{}", self.id, self.path)
    }
}
