//! Plugin registry index — central catalog of available plugins.
//!
//! The registry index is a JSON file hosted at a URL. It lists all
//! available plugins with metadata and download URLs. `magistr plugins install`
//! fetches this index to resolve plugin names to downloadable archives.

use serde::{Deserialize, Serialize};

use super::types::{PluginError, PluginKind};

/// Default URL for the official Magistr plugin registry index.
pub const DEFAULT_INDEX_URL: &str = "https://plugins.magistr.dev/index.json";

/// Registry index — the central catalog of available plugins.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RegistryIndex {
    /// Schema version of the index format.
    pub version: u32,
    /// Available plugins.
    pub plugins: Vec<IndexEntry>,
}

/// A single plugin entry in the registry index.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct IndexEntry {
    /// Unique plugin name.
    pub name: String,
    /// Plugin version (semver string).
    pub version: String,
    /// What kind of plugin this is.
    pub kind: PluginKind,
    /// Short description.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// URL to download the plugin archive (`.tar.gz`).
    pub download_url: String,
    /// Optional base64-encoded ed25519 signature of the archive.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub signature: Option<String>,
    /// Author name or organization.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub author: Option<String>,
}

impl RegistryIndex {
    /// Parse an index from JSON content.
    pub fn from_json(content: &str) -> Result<Self, PluginError> {
        serde_json::from_str(content).map_err(|e| PluginError::IndexParse(e.to_string()))
    }

    /// Serialize the index to JSON.
    pub fn to_json(&self) -> Result<String, PluginError> {
        serde_json::to_string_pretty(self).map_err(|e| PluginError::IndexParse(e.to_string()))
    }

    /// Find a plugin by exact name.
    #[must_use]
    pub fn find(&self, name: &str) -> Option<&IndexEntry> {
        self.plugins.iter().find(|e| e.name == name)
    }

    /// List all plugin names.
    #[must_use]
    pub fn names(&self) -> Vec<&str> {
        self.plugins.iter().map(|e| e.name.as_str()).collect()
    }

    /// Fetch the registry index from a URL.
    pub fn fetch(url: &str) -> Result<Self, PluginError> {
        let body: String = ureq::get(url)
            .call()
            .map_err(|e| PluginError::IndexFetch(e.to_string()))?
            .body_mut()
            .read_to_string()
            .map_err(|e| PluginError::IndexFetch(e.to_string()))?;

        Self::from_json(&body)
    }

    /// Fetch the index from the default URL.
    pub fn fetch_default() -> Result<Self, PluginError> {
        Self::fetch(DEFAULT_INDEX_URL)
    }
}

/// Download a plugin archive from a URL and return the raw bytes.
pub fn download_archive(url: &str) -> Result<Vec<u8>, PluginError> {
    let bytes = ureq::get(url)
        .call()
        .map_err(|e| PluginError::DownloadFailed(e.to_string()))?
        .body_mut()
        .read_to_vec()
        .map_err(|e| PluginError::DownloadFailed(e.to_string()))?;

    Ok(bytes)
}

/// Extract a `.tar.gz` archive into a destination directory.
pub fn extract_archive(archive_bytes: &[u8], dest: &std::path::Path) -> Result<(), PluginError> {
    use flate2::read::GzDecoder;
    use tar::Archive;

    let decoder = GzDecoder::new(archive_bytes);
    let mut archive = Archive::new(decoder);

    archive
        .unpack(dest)
        .map_err(|e| PluginError::ArchiveError(e.to_string()))?;

    Ok(())
}

#[cfg(test)]
#[path = "index.tests.rs"]
mod tests;
