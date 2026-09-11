use serde::Serialize;

/// Build roles established by a member's manifests and source tree.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum MapCrateRole {
    Lib,
    Bin,
    ProcMacro,
    Build,
}
