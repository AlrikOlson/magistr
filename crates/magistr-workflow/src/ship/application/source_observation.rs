use serde::{Deserialize, Serialize};

use super::ObservationDrift;

/// What the host's source capability observed when a check was admitted:
/// which workspace (canonical root and the owner generation then durable),
/// under which hashing scheme and selection policy, the content-addressed
/// revision of the selected bytes, the digest of those bytes alone, the
/// digest of the applicable external configuration layers, and what that
/// observation may claim (`optimistic` for a live tree, `immutable` for an
/// isolated export). The manifest and the layer contents stay with the
/// capability; the workflow keeps only what a later comparison needs.
///
/// The identities are kept apart on purpose: `revision` is the workspace's
/// own name for the observation (it folds the owner generation in),
/// `source_digest` names the selected bytes, `policy_digest` the selection
/// they were taken under and `config_digest` the policy that applied
/// outside the tree. A later comparison ([`Self::drift_to`]) judges each
/// one and never treats a new owner generation as a change of source.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct SourceObservation {
    pub workspace_root: String,
    pub workspace_generation: Option<u64>,
    pub scheme: String,
    pub policy_digest: String,
    pub revision: String,
    /// The selected bytes under the policy, content-addressed.
    pub source_digest: String,
    /// The applicable external configuration layers, by content.
    pub config_digest: String,
    pub consistency: String,
    pub captured_at_ms: u64,
}

impl SourceObservation {
    /// The first distinguished identity on which a later observation of
    /// the same capability disagrees with this one, or `None` when the
    /// evidence this observation was taken for still applies. Judged in
    /// order: workspace, scheme, selection policy, source bytes, external
    /// configuration. The owner generation is carried, not judged: a new
    /// owner of the same bytes is not a change of source.
    #[must_use]
    pub fn drift_to(&self, later: &Self) -> Option<ObservationDrift> {
        if self.workspace_root != later.workspace_root {
            return Some(ObservationDrift::ForeignWorkspace {
                expected: self.workspace_root.clone(),
                actual: later.workspace_root.clone(),
            });
        }
        if self.scheme != later.scheme {
            return Some(ObservationDrift::Scheme {
                before: self.scheme.clone(),
                after: later.scheme.clone(),
            });
        }
        if self.policy_digest != later.policy_digest {
            return Some(ObservationDrift::SelectionPolicy {
                before: self.policy_digest.clone(),
                after: later.policy_digest.clone(),
            });
        }
        if self.source_digest.is_empty() {
            return Some(ObservationDrift::Unbound {
                identity: "source".into(),
            });
        }
        if self.source_digest != later.source_digest {
            return Some(ObservationDrift::Source {
                before: self.source_digest.clone(),
                after: later.source_digest.clone(),
            });
        }
        if self.config_digest.is_empty() {
            return Some(ObservationDrift::Unbound {
                identity: "configuration".into(),
            });
        }
        if self.config_digest != later.config_digest {
            return Some(ObservationDrift::Configuration {
                before: self.config_digest.clone(),
                after: later.config_digest.clone(),
            });
        }
        None
    }
}
