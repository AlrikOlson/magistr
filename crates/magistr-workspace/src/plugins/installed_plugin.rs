use serde_json::Value;

use super::{PluginSource, PluginTier};

/// One `.magistr/workspace/plugins.json` entry as the host reads it. `dir` and
/// `grants` make an entry invocable; both fail closed — a malformed value
/// means inventory-only / nothing granted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstalledPluginRecord {
    pub name: String,
    pub version: Option<String>,
    pub source: PluginSource,
    pub tier: Option<PluginTier>,
    /// Workspace-relative directory holding plugin.toml and the component.
    pub dir: Option<String>,
    /// Install-time granted import names. Default deny: absent grants nothing.
    pub grants: Option<Vec<String>>,
}

impl InstalledPluginRecord {
    /// Decode one inventory entry with the retained fail-closed rules: a
    /// source other than exactly "registry" is a sideload, and a tier is
    /// honored only on registry entries.
    pub fn from_value(entry: &Value) -> Result<Self, String> {
        let name = entry
            .get("name")
            .and_then(Value::as_str)
            .filter(|name| !name.is_empty())
            .ok_or_else(|| {
                ".magistr/workspace/plugins.json: every plugin needs a non-empty name".to_owned()
            })?;
        let source = match entry.get("source").and_then(Value::as_str) {
            Some("registry") => PluginSource::Registry,
            _ => PluginSource::Sideload,
        };
        let tier = match (source, entry.get("tier").and_then(Value::as_str)) {
            (PluginSource::Registry, Some("blessed")) => Some(PluginTier::Blessed),
            (PluginSource::Registry, Some("verified")) => Some(PluginTier::Verified),
            _ => None,
        };
        let dir = entry
            .get("dir")
            .and_then(Value::as_str)
            .filter(|dir| !dir.is_empty() && !dir.contains(".."))
            .map(str::to_owned);
        let grants = entry
            .get("grants")
            .and_then(Value::as_array)
            .and_then(|list| {
                list.iter()
                    .map(|grant| grant.as_str().map(str::to_owned))
                    .collect::<Option<Vec<_>>>()
            });
        Ok(Self {
            name: name.to_owned(),
            version: entry
                .get("version")
                .and_then(Value::as_str)
                .map(str::to_owned),
            source,
            tier,
            dir,
            grants,
        })
    }

    /// The `pkg` half of `publisher.pkg`, the discovery key for lane conventions.
    #[must_use]
    pub fn pkg(&self) -> &str {
        self.name
            .split_once('.')
            .map_or(self.name.as_str(), |(_, pkg)| pkg)
    }

    #[must_use]
    pub fn is_blessed(&self) -> bool {
        self.tier == Some(PluginTier::Blessed)
    }
}
