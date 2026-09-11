use serde::Serialize;

/// Manifest or conventional role of a workspace entry point.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum MapEntryKind {
    PackageMain,
    PackageBin,
    CargoBin,
    Convention,
    Lib,
    Bin,
    Example,
    Test,
    Bench,
    Build,
}
