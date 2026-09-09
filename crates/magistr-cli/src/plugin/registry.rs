//! Plugin registry — manage installed plugins.
//!
//! The registry tracks which plugins are installed, stores them in
//! `~/.config/magistr/plugins/`, and provides list/add/remove operations.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::index::{self, IndexEntry};
use super::loader;
use super::signing;
use super::types::{InstalledPlugin, PluginError, PluginManifest, PluginSource};

/// Default plugin storage directory relative to config dir.
const PLUGINS_DIR: &str = "plugins";

/// Registry metadata filename.
const REGISTRY_FILENAME: &str = "registry.json";

/// Resolve the plugins directory path.
///
/// Uses `$XDG_CONFIG_HOME/magistr/plugins/` if set, otherwise `~/.config/magistr/plugins/`.
pub fn plugins_dir() -> Option<PathBuf> {
    config_dir().map(|d| d.join(PLUGINS_DIR))
}

/// Resolve the registry file path.
pub fn registry_path() -> Option<PathBuf> {
    plugins_dir().map(|d| d.join(REGISTRY_FILENAME))
}

/// Resolve the config directory.
fn config_dir() -> Option<PathBuf> {
    if let Ok(xdg) = std::env::var("XDG_CONFIG_HOME") {
        return Some(PathBuf::from(xdg).join("magistr"));
    }
    std::env::var("HOME")
        .ok()
        .map(|home| PathBuf::from(home).join(".config/magistr"))
}

/// Org-managed plugin registry configuration.
///
/// Loaded from `~/.config/magistr/org-plugins.json` or set via environment
/// variable `MAGISTR_PLUGIN_INDEX_URL`. Allows organizations to:
/// - Use a private plugin index URL instead of the default public one
/// - Restrict which plugins are allowed to be installed (allowlist)
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct OrgPluginConfig {
    /// Private plugin index URL. If set, replaces the default public index.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub index_url: Option<String>,
    /// Plugin allowlist. If non-empty, only plugins in this list can be installed.
    /// Each entry is a plugin name (e.g., "license-gate", "sonarqube-gate").
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub allowlist: Vec<String>,
    /// Ed25519 public key for verifying plugin signatures.
    /// If set, all plugins from the private index must be signed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub signing_key: Option<String>,
}

/// Filename for org plugin configuration.
const ORG_CONFIG_FILENAME: &str = "org-plugins.json";

impl OrgPluginConfig {
    /// Load org config from disk, environment, or defaults.
    ///
    /// Priority:
    /// 1. `MAGISTR_PLUGIN_INDEX_URL` env var (index URL only)
    /// 2. `~/.config/magistr/org-plugins.json` file
    /// 3. Default (no restrictions)
    pub fn load() -> Self {
        let mut config = Self::load_file().unwrap_or_default();

        // Environment variable overrides file config for index URL
        if let Ok(url) = std::env::var("MAGISTR_PLUGIN_INDEX_URL") {
            config.index_url = Some(url);
        }

        config
    }

    /// Load from the config file.
    fn load_file() -> Option<Self> {
        let path = config_dir()?.join(ORG_CONFIG_FILENAME);
        let content = std::fs::read_to_string(path).ok()?;
        serde_json::from_str(&content).ok()
    }

    /// Save to the config file.
    pub fn save(&self) -> Result<(), PluginError> {
        let dir = config_dir().ok_or_else(|| {
            PluginError::Io(
                "cannot resolve config directory".to_string(),
                std::io::Error::new(std::io::ErrorKind::NotFound, "no config dir"),
            )
        })?;
        std::fs::create_dir_all(&dir).map_err(|e| PluginError::Io(dir.display().to_string(), e))?;

        let path = dir.join(ORG_CONFIG_FILENAME);
        let content = serde_json::to_string_pretty(self)
            .map_err(|e| PluginError::ManifestSerialize(e.to_string()))?;
        std::fs::write(&path, content)
            .map_err(|e| PluginError::Io(path.display().to_string(), e))?;

        Ok(())
    }

    /// Get the effective index URL (private or default).
    #[must_use]
    pub fn effective_index_url(&self) -> &str {
        self.index_url
            .as_deref()
            .unwrap_or(super::index::DEFAULT_INDEX_URL)
    }

    /// Check if a plugin name is allowed by the org policy.
    ///
    /// Returns `true` if the allowlist is empty (no restrictions) or
    /// if the plugin name is in the allowlist.
    #[must_use]
    pub fn is_allowed(&self, plugin_name: &str) -> bool {
        self.allowlist.is_empty() || self.allowlist.iter().any(|a| a == plugin_name)
    }

    /// Check if the allowlist is active (non-empty).
    #[must_use]
    pub fn has_allowlist(&self) -> bool {
        !self.allowlist.is_empty()
    }

    /// Get the set of allowed plugin names.
    #[must_use]
    pub fn allowed_set(&self) -> HashSet<&str> {
        self.allowlist.iter().map(|s| s.as_str()).collect()
    }
}

/// Plugin registry — manages installed plugins.
#[derive(Debug, Clone)]
pub struct PluginRegistry {
    /// All installed plugins.
    plugins: Vec<InstalledPlugin>,
    /// Base directory where plugins are stored.
    base_dir: PathBuf,
    /// Org-managed configuration (allowlist + private index).
    org_config: OrgPluginConfig,
}

impl PluginRegistry {
    /// Create a new registry at the given base directory.
    pub fn new(base_dir: &Path) -> Self {
        Self {
            plugins: Vec::new(),
            base_dir: base_dir.to_path_buf(),
            org_config: OrgPluginConfig::default(),
        }
    }

    /// Create a new registry with org configuration.
    pub fn with_org_config(base_dir: &Path, org_config: OrgPluginConfig) -> Self {
        Self {
            plugins: Vec::new(),
            base_dir: base_dir.to_path_buf(),
            org_config,
        }
    }

    /// Load registry from disk.
    ///
    /// Returns an empty registry if the file doesn't exist.
    /// Also loads org-managed configuration.
    pub fn load(base_dir: &Path) -> Result<Self, PluginError> {
        let registry_path = base_dir.join(REGISTRY_FILENAME);

        if !registry_path.is_file() {
            return Ok(Self::with_org_config(base_dir, OrgPluginConfig::load()));
        }

        let content = std::fs::read_to_string(&registry_path)
            .map_err(|e| PluginError::Io(registry_path.display().to_string(), e))?;

        let plugins: Vec<InstalledPlugin> = serde_json::from_str(&content)
            .map_err(|e| PluginError::ManifestParse(e.to_string()))?;

        Ok(Self {
            plugins,
            base_dir: base_dir.to_path_buf(),
            org_config: OrgPluginConfig::load(),
        })
    }

    /// Get the org-managed configuration.
    #[must_use]
    pub fn org_config(&self) -> &OrgPluginConfig {
        &self.org_config
    }

    /// Set the org-managed configuration.
    pub fn set_org_config(&mut self, config: OrgPluginConfig) {
        self.org_config = config;
    }

    /// Save registry to disk.
    pub fn save(&self) -> Result<(), PluginError> {
        std::fs::create_dir_all(&self.base_dir)
            .map_err(|e| PluginError::Io(self.base_dir.display().to_string(), e))?;

        let registry_path = self.base_dir.join(REGISTRY_FILENAME);
        let content = serde_json::to_string_pretty(&self.plugins)
            .map_err(|e| PluginError::ManifestSerialize(e.to_string()))?;

        std::fs::write(&registry_path, content)
            .map_err(|e| PluginError::Io(registry_path.display().to_string(), e))?;

        Ok(())
    }

    /// List all installed plugins.
    pub fn list(&self) -> &[InstalledPlugin] {
        &self.plugins
    }

    /// Get a plugin by name.
    pub fn get(&self, name: &str) -> Option<&InstalledPlugin> {
        self.plugins.iter().find(|p| p.manifest.name == name)
    }

    /// Check if a plugin is installed.
    pub fn is_installed(&self, name: &str) -> bool {
        self.plugins.iter().any(|p| p.manifest.name == name)
    }

    /// Install a plugin from a local directory.
    ///
    /// Copies the plugin directory into the registry's storage,
    /// then registers it. Returns an error if already installed.
    pub fn add_local(&mut self, source_dir: &Path) -> Result<&InstalledPlugin, PluginError> {
        let manifest = loader::load_manifest(source_dir)?;

        // Enforce org allowlist
        if !self.org_config.is_allowed(&manifest.name) {
            return Err(PluginError::NotFound(format!(
                "plugin '{}' is not in the organization allowlist",
                manifest.name
            )));
        }

        if self.is_installed(&manifest.name) {
            return Err(PluginError::AlreadyInstalled(manifest.name));
        }

        // Copy plugin directory to storage
        let dest_dir = self.base_dir.join(&manifest.name);
        copy_dir_recursive(source_dir, &dest_dir)?;

        let installed = InstalledPlugin {
            manifest,
            source: PluginSource::Local {
                path: source_dir.display().to_string(),
            },
            installed_at: chrono::Utc::now().to_rfc3339(),
        };

        self.plugins.push(installed);
        // Return reference to the just-pushed element
        Ok(self.plugins.last().expect("just pushed"))
    }

    /// Install a plugin from the registry index.
    ///
    /// Downloads the plugin archive, optionally verifies its signature,
    /// extracts it, validates the manifest, and registers it.
    pub fn install_from_index(
        &mut self,
        entry: &IndexEntry,
        public_key: Option<&str>,
    ) -> Result<&InstalledPlugin, PluginError> {
        // Enforce org allowlist
        if !self.org_config.is_allowed(&entry.name) {
            return Err(PluginError::NotFound(format!(
                "plugin '{}' is not in the organization allowlist",
                entry.name
            )));
        }

        if self.is_installed(&entry.name) {
            return Err(PluginError::AlreadyInstalled(entry.name.clone()));
        }

        // Download the archive
        let archive_bytes = index::download_archive(&entry.download_url)?;

        // Verify signature if present and public key provided
        if let (Some(sig), Some(key)) = (&entry.signature, public_key) {
            signing::verify_signature(&archive_bytes, sig, key)?;
        }

        // Extract to a temp directory first, then move into place
        let temp_dir = std::env::temp_dir().join(format!("magistr-plugin-{}", entry.name));
        if temp_dir.exists() {
            std::fs::remove_dir_all(&temp_dir)
                .map_err(|e| PluginError::Io(temp_dir.display().to_string(), e))?;
        }
        std::fs::create_dir_all(&temp_dir)
            .map_err(|e| PluginError::Io(temp_dir.display().to_string(), e))?;

        index::extract_archive(&archive_bytes, &temp_dir)?;

        // The archive may contain a single top-level directory or files directly.
        // Find the manifest to determine the actual plugin root.
        let plugin_root = find_plugin_root(&temp_dir)?;

        // Validate the extracted plugin
        loader::load_manifest(&plugin_root)?;

        // Copy to the registry storage
        let dest_dir = self.base_dir.join(&entry.name);
        copy_dir_recursive(&plugin_root, &dest_dir)?;

        // Clean up temp directory (best effort)
        let _ = std::fs::remove_dir_all(&temp_dir);

        let manifest = loader::load_manifest(&dest_dir)?;

        let installed = InstalledPlugin {
            manifest,
            source: PluginSource::Registry {
                name: entry.name.clone(),
                version: entry.version.clone(),
            },
            installed_at: chrono::Utc::now().to_rfc3339(),
        };

        self.plugins.push(installed);
        Ok(self.plugins.last().expect("just pushed"))
    }

    /// Remove a plugin by name.
    ///
    /// Deletes the plugin directory from storage and unregisters it.
    pub fn remove(&mut self, name: &str) -> Result<InstalledPlugin, PluginError> {
        let idx = self
            .plugins
            .iter()
            .position(|p| p.manifest.name == name)
            .ok_or_else(|| PluginError::NotFound(name.to_string()))?;

        // Remove plugin directory
        let plugin_dir = self.base_dir.join(name);
        if plugin_dir.is_dir() {
            std::fs::remove_dir_all(&plugin_dir)
                .map_err(|e| PluginError::Io(plugin_dir.display().to_string(), e))?;
        }

        Ok(self.plugins.remove(idx))
    }

    /// Get the directory path for an installed plugin.
    pub fn plugin_dir(&self, name: &str) -> PathBuf {
        self.base_dir.join(name)
    }

    /// Number of installed plugins.
    pub fn len(&self) -> usize {
        self.plugins.len()
    }

    /// Whether the registry is empty.
    pub fn is_empty(&self) -> bool {
        self.plugins.is_empty()
    }

    /// Register a plugin directly (for testing or manual installs).
    ///
    /// Does not copy files — assumes the plugin directory already exists.
    /// Enforces the org allowlist if configured.
    pub fn register(
        &mut self,
        manifest: PluginManifest,
        source: PluginSource,
    ) -> Result<(), PluginError> {
        // Enforce org allowlist
        if !self.org_config.is_allowed(&manifest.name) {
            return Err(PluginError::NotFound(format!(
                "plugin '{}' is not in the organization allowlist",
                manifest.name
            )));
        }

        if self.is_installed(&manifest.name) {
            return Err(PluginError::AlreadyInstalled(manifest.name));
        }

        self.plugins.push(InstalledPlugin {
            manifest,
            source,
            installed_at: chrono::Utc::now().to_rfc3339(),
        });

        Ok(())
    }

    /// Fetch the plugin index using the org-configured URL.
    ///
    /// Uses the private index URL if configured, otherwise the default
    /// public index.
    pub fn fetch_index(&self) -> Result<super::index::RegistryIndex, PluginError> {
        super::index::RegistryIndex::fetch(self.org_config.effective_index_url())
    }

    /// Install a plugin by name from the org-configured index.
    ///
    /// Fetches the index, finds the plugin, and installs it with
    /// signature verification using the org signing key if configured.
    pub fn install_by_name(&mut self, name: &str) -> Result<&InstalledPlugin, PluginError> {
        let index = self.fetch_index()?;
        let entry = index
            .find(name)
            .ok_or_else(|| PluginError::IndexNotFound(name.to_string()))?
            .clone();

        let public_key = self.org_config.signing_key.clone();
        self.install_from_index(&entry, public_key.as_deref())
    }
}

/// Find the plugin root directory inside an extracted archive.
///
/// If the archive extracted a single top-level directory containing `magistr-plugin.toml`,
/// return that directory. Otherwise, check the root itself.
fn find_plugin_root(extracted: &Path) -> Result<PathBuf, PluginError> {
    use super::types::MANIFEST_FILENAME;

    // Check if manifest is directly in the extracted root
    if extracted.join(MANIFEST_FILENAME).is_file() {
        return Ok(extracted.to_path_buf());
    }

    // Check for a single subdirectory containing the manifest
    let entries: Vec<_> = std::fs::read_dir(extracted)
        .map_err(|e| PluginError::Io(extracted.display().to_string(), e))?
        .filter_map(|e| e.ok())
        .filter(|e| e.path().is_dir())
        .collect();

    if entries.len() == 1 {
        let subdir = entries[0].path();
        if subdir.join(MANIFEST_FILENAME).is_file() {
            return Ok(subdir);
        }
    }

    Err(PluginError::ArchiveError(format!(
        "no {MANIFEST_FILENAME} found in extracted archive"
    )))
}

/// Recursively copy a directory.
fn copy_dir_recursive(src: &Path, dest: &Path) -> Result<(), PluginError> {
    std::fs::create_dir_all(dest).map_err(|e| PluginError::Io(dest.display().to_string(), e))?;

    let entries =
        std::fs::read_dir(src).map_err(|e| PluginError::Io(src.display().to_string(), e))?;

    for entry in entries {
        let entry = entry.map_err(|e| PluginError::Io(src.display().to_string(), e))?;
        let src_path = entry.path();
        let dest_path = dest.join(entry.file_name());

        if src_path.is_dir() {
            copy_dir_recursive(&src_path, &dest_path)?;
        } else {
            std::fs::copy(&src_path, &dest_path)
                .map_err(|e| PluginError::Io(src_path.display().to_string(), e))?;
        }
    }

    Ok(())
}

#[cfg(test)]
#[path = "registry.tests.rs"]
mod tests;
