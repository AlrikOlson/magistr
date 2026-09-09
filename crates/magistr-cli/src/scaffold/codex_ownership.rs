use super::managed_content::ManagedContent;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// Codex owns individual config values and one instruction block, never shared files.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub(crate) struct CodexOwnership {
    #[serde(default)]
    pub(crate) selected: bool,
    #[serde(default)]
    pub(super) files: BTreeSet<String>,
    #[serde(default)]
    pub(super) config: BTreeMap<String, ManagedContent>,
    #[serde(default)]
    pub(super) agents: Option<ManagedContent>,
}
