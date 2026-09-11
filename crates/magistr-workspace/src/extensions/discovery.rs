use std::path::{Path, PathBuf};

use super::{InstalledPluginRecord, PluginInventory, PluginManifest, ResolvedPlugin};
use crate::workspace_policy::WorkspacePolicy;

/// Ceiling on one manifest's text.
const MANIFEST_MAX_BYTES: u64 = 64 * 1024;

/// Stateless plugin discovery shared by every lane: the inventory and each
/// manifest are read per call through the workspace jail, and every refusal
/// is a note rather than a failure of unrelated tools.
pub(super) struct Discovery<'a> {
    pub policy: &'a WorkspacePolicy,
    pub state_dir: PathBuf,
}

impl Discovery<'_> {
    pub fn inventory(&self) -> Result<Vec<InstalledPluginRecord>, String> {
        PluginInventory::load(&self.state_dir)
    }

    /// The manifest text of an installed plugin directory, jail-checked.
    pub fn read_manifest(&self, dir: &str) -> Result<String, String> {
        let relative = format!("{}/plugin.toml", dir.trim_end_matches('/'));
        let admitted = self
            .policy
            .check_read(&relative)
            .map_err(|error| error.to_string())?;
        read_bounded(&admitted)
    }

    /// Every installed plugin with a readable, valid manifest.
    pub fn resolve_all(&self) -> (Vec<ResolvedPlugin>, Vec<String>) {
        let mut notes = Vec::new();
        let inventory = match self.inventory() {
            Ok(inventory) => inventory,
            Err(error) => {
                notes.push(format!("plugin inventory unreadable: {error}"));
                return (Vec::new(), notes);
            }
        };
        let mut plugins = Vec::new();
        for record in inventory {
            let Some(dir) = record.dir.as_deref() else {
                continue;
            };
            let source = match self.read_manifest(dir) {
                Ok(source) => source,
                Err(_) => {
                    notes.push(format!(
                        "plugin {}: plugin.toml unreadable — not offered",
                        record.name
                    ));
                    continue;
                }
            };
            match PluginManifest::parse(&source) {
                Ok(parsed) => {
                    notes.extend(
                        parsed
                            .warnings
                            .iter()
                            .map(|warning| format!("plugin {}: {warning}", record.name)),
                    );
                    plugins.push(ResolvedPlugin {
                        record,
                        manifest: parsed.manifest,
                    });
                }
                Err(errors) => notes.push(format!(
                    "plugin {}: invalid manifest ({}) — not offered",
                    record.name,
                    errors.first().map_or("", String::as_str)
                )),
            }
        }
        (plugins, notes)
    }
}

fn read_bounded(path: &Path) -> Result<String, String> {
    let metadata = std::fs::metadata(path).map_err(|error| error.to_string())?;
    if !metadata.is_file() {
        return Err("plugin.toml is not a regular file".into());
    }
    if metadata.len() > MANIFEST_MAX_BYTES {
        return Err("plugin.toml exceeds the 64 KiB manifest bound".into());
    }
    std::fs::read_to_string(path).map_err(|error| error.to_string())
}
