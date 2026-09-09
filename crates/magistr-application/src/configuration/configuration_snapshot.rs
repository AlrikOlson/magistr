use magistr_domain::identity::ProjectId;

use crate::Revision;

use super::ProjectSettings;

/// Resolved configuration plus the authoritative project revision used for a patch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigurationSnapshot {
    pub project: ProjectId,
    pub revision: Revision,
    pub settings: ProjectSettings,
}
