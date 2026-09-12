use super::SourceChange;

/// The answer to "does the workspace still hold the bytes this snapshot
/// observed?". `Unchanged` is itself an observation: the later read matched
/// the earlier one. It is not evidence that nothing happened in between and
/// never upgrades an optimistic snapshot to immutable inputs.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SnapshotValidation {
    Unchanged,
    Changed(Vec<SourceChange>),
}

impl SnapshotValidation {
    #[must_use]
    pub const fn is_unchanged(&self) -> bool {
        matches!(self, Self::Unchanged)
    }
}
