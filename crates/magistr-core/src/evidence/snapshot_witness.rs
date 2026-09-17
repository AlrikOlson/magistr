use std::path::PathBuf;

use magistr_engine::native::{SourceFreshness, SourceObservation, SourceWitness};
use magistr_workspace::config::{ConfigDigest, ConfigLayer};
use magistr_workspace::snapshot::{SnapshotValidation, SourceSnapshot};

/// One optimistic observation of the workspace and of the external
/// configuration layers that applied to it, held until the check it was
/// taken for settles. Validation re-runs the snapshot's own policy against
/// the same root and names the paths that differ; with the bytes agreed,
/// it re-reads the same layers and names the ones that differ.
pub struct SnapshotWitness {
    root: PathBuf,
    snapshot: SourceSnapshot,
    layers: Vec<(ConfigLayer, PathBuf)>,
    configuration: ConfigDigest,
}

impl SnapshotWitness {
    #[must_use]
    pub fn new(
        root: PathBuf,
        snapshot: SourceSnapshot,
        layers: Vec<(ConfigLayer, PathBuf)>,
        configuration: ConfigDigest,
    ) -> Self {
        Self {
            root,
            snapshot,
            layers,
            configuration,
        }
    }

    /// The snapshot as captured.
    #[must_use]
    pub fn snapshot(&self) -> &SourceSnapshot {
        &self.snapshot
    }

    /// The external configuration as captured.
    #[must_use]
    pub fn configuration(&self) -> &ConfigDigest {
        &self.configuration
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
            source_digest: self.snapshot.manifest.digest(),
            config_digest: self.configuration.digest(),
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
            SnapshotValidation::Unchanged => {}
            SnapshotValidation::Changed(changes) => {
                return Ok(SourceFreshness::Changed {
                    paths: changes
                        .iter()
                        .map(|change| format!("{} ({:?})", change.path, change.kind).to_owned())
                        .collect(),
                });
            }
        }
        let configuration =
            ConfigDigest::observe(&self.layers).map_err(|error| error.to_string())?;
        let layers = self.configuration.changes_to(&configuration);
        Ok(if layers.is_empty() {
            SourceFreshness::Unchanged
        } else {
            SourceFreshness::Reconfigured {
                layers: layers
                    .into_iter()
                    .map(|layer| layer.as_str().to_owned())
                    .collect(),
            }
        })
    }
}
