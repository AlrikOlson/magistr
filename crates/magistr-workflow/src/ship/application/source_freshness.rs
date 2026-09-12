use serde::{Deserialize, Serialize};

/// Whether the workspace still held the bytes a check was admitted against
/// when its result came back. `Unchanged` is the agreement of two reads,
/// never proof that nothing happened between them.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "freshness", rename_all = "snake_case")]
pub enum SourceFreshness {
    /// The settlement read matched the admission read.
    Unchanged,
    /// These paths differed between the two reads.
    Changed { paths: Vec<String> },
    /// The revisions differ and the paths are unknown: the admission's
    /// witness was not available at settlement (another process admitted
    /// the check), so only the content-addressed revisions were compared.
    Diverged { before: String, after: String },
    /// The workspace could not be re-observed at settlement.
    Unobserved { reason: String },
}

impl SourceFreshness {
    #[must_use]
    pub const fn is_unchanged(&self) -> bool {
        matches!(self, Self::Unchanged)
    }
}
