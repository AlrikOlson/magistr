use serde::{Deserialize, Serialize};

/// Whether the workspace still held the bytes — under the configuration —
/// a check was admitted against when its result came back. `Unchanged` is
/// the agreement of two reads, never proof that nothing happened between
/// them.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(tag = "freshness", rename_all = "snake_case")]
pub enum SourceFreshness {
    /// The settlement read matched the admission read.
    Unchanged,
    /// These paths differed between the two reads.
    Changed { paths: Vec<String> },
    /// The source digests differ and the paths are unknown: the
    /// admission's witness was not available at settlement (another
    /// process admitted the check), so only the content digests were
    /// compared.
    Diverged { before: String, after: String },
    /// The selected bytes agreed but an applicable external configuration
    /// layer changed between the two reads; the layers are named when the
    /// admitting witness settled the check, empty when only the digests
    /// could be compared.
    Reconfigured { layers: Vec<String> },
    /// The workspace could not be re-observed at settlement.
    Unobserved { reason: String },
}

impl SourceFreshness {
    #[must_use]
    pub const fn is_unchanged(&self) -> bool {
        matches!(self, Self::Unchanged)
    }
}
