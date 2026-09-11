use super::PluginManifest;

/// A validated manifest with the non-fatal observations made while parsing it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedManifest {
    pub manifest: PluginManifest,
    pub warnings: Vec<String>,
}
