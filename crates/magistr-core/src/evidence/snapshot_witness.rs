use std::path::PathBuf;

use magistr_engine::tas::{SourceFreshness, SourceObservation, SourceWitness};
use magistr_workspace::snapshot::{SnapshotValidation, SourceSnapshot};

/// One optimistic observation of the workspace, held until the check it
/// was taken for settles. Validation re-runs the snapshot's own policy
/// against the same root and names the paths that differ.
pub struct SnapshotWitness {
    root: PathBuf,
    snapshot: SourceSnapshot,
}

impl SnapshotWitness {
    #[must_use]
    pub fn new(root: PathBuf, snapshot: SourceSnapshot) -> Self {
        Self { root, snapshot }
    }

    /// The snapshot as captured.
    #[must_use]
    pub fn snapshot(&self) -> &SourceSnapshot {
        &self.snapshot
    }
}

impl SourceWitness for SnapshotWitness {
    fn observation(&self) -> SourceObservation {
        SourceObservation {
            workspace_root: self.snapshot.identity.root.display().to_string(),
            workspace_generation: self.snapshot.identity.generation,
            scheme: self.snapshot.scheme.clone(),
            policy_digest: self.snapshot.policy_digest.clone(),
            revision: self.snapshot.revision(),
            consistency: self.snapshot.consistency.as_str().to_owned(),
            captured_at_ms: self.snapshot.captured_at_ms,
        }
    }

    fn validate(&self) -> Result<SourceFreshness, String> {
        match self
            .snapshot
            .validate(&self.root)
            .map_err(|error| error.to_string())?
        {
            SnapshotValidation::Unchanged => Ok(SourceFreshness::Unchanged),
            SnapshotValidation::Changed(changes) => Ok(SourceFreshness::Changed {
                paths: changes
                    .iter()
                    .map(|change| format!("{} ({:?})", change.path, change.kind).to_owned())
                    .collect(),
            }),
        }
    }
}
