use super::{InstalledPluginRecord, PluginManifest};

/// An installed record whose manifest parsed: the unit every lane discovers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedPlugin {
    pub record: InstalledPluginRecord,
    pub manifest: PluginManifest,
}
