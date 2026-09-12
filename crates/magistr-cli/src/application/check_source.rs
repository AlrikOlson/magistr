//! What the workspace looked like before a check ran, and whether it still did after.

use magistr_workspace::snapshot::{SnapshotValidation, SourceSnapshot};

/// The source evidence of one settled check: the optimistic observation
/// taken at admission and the re-observation taken at settlement. An
/// `Unchanged` validation says the two reads agreed; it never certifies
/// that nothing happened between them, and the snapshot's consistency
/// stays `Optimistic` — the receipts chunk decides what a consumer may
/// claim from it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CheckSource {
    pub before: SourceSnapshot,
    pub validation: SnapshotValidation,
}

impl CheckSource {
    /// The content-addressed revision the check was admitted against.
    #[must_use]
    pub fn revision(&self) -> String {
        self.before.revision()
    }

    /// Whether the settlement read matched the admission read.
    #[must_use]
    pub const fn is_unchanged(&self) -> bool {
        self.validation.is_unchanged()
    }
}
