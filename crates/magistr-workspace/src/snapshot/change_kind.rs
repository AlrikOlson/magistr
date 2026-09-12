use serde::{Deserialize, Serialize};

/// How one path differs between two manifests.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChangeKind {
    Added,
    Removed,
    /// Kind, mode, size or digest differ.
    Modified,
}
