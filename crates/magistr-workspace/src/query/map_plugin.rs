use std::path::Path;

use serde::Serialize;

use crate::plugins::{InstalledPluginRecord, PluginInventory, PluginSource, PluginTier};

/// Installed-plugin inventory row; trust is a composition-time fact.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct MapPlugin {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    pub source: String,
    pub trust: String,
}

impl MapPlugin {
    /// The inventory as map rows, read from `<state>/plugins.json` on every
    /// call through the one shared parser. Trust follows the retained
    /// fail-closed rule the records already encode: a sideload is always
    /// untrusted (it cannot smuggle a tier in); a registry entry wears its
    /// signed tier, or "verified" without one. A missing file is an empty
    /// inventory; an unreadable one is a note, never a failure.
    #[must_use]
    pub fn inventory(state_dir: &Path) -> (Vec<Self>, Vec<String>) {
        let records = match PluginInventory::load(state_dir) {
            Ok(records) => records,
            Err(_) => {
                return (
                    Vec::new(),
                    vec!["plugin inventory unreadable; no plugins are shown".to_owned()],
                );
            }
        };
        let plugins: Vec<Self> = records.iter().map(Self::from_record).collect();
        let untrusted: Vec<&str> = plugins
            .iter()
            .filter(|plugin| plugin.trust == "untrusted")
            .map(|plugin| plugin.name.as_str())
            .collect();
        let notes = if untrusted.is_empty() {
            Vec::new()
        } else {
            vec![format!(
                "{} sideloaded plugin(s) are untrusted: {} (SDD §8.4 — tightest capability defaults)",
                untrusted.len(),
                untrusted.join(", ")
            )]
        };
        (plugins, notes)
    }

    fn from_record(record: &InstalledPluginRecord) -> Self {
        let trust = match (record.source, record.tier) {
            (PluginSource::Sideload, _) => "untrusted",
            (PluginSource::Registry, Some(PluginTier::Blessed)) => PluginTier::Blessed.as_str(),
            (PluginSource::Registry, _) => PluginTier::Verified.as_str(),
        };
        Self {
            name: record.name.clone(),
            version: record.version.clone(),
            source: record.source.as_str().to_owned(),
            trust: trust.to_owned(),
        }
    }
}

#[cfg(test)]
#[path = "map_plugin.tests.rs"]
mod tests;
