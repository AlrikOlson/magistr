use std::path::Path;

use super::InstalledPluginRecord;

/// The installed-plugin inventory, read from `<state>/plugins.json` on every
/// call — stateless by construction. A missing file is an empty inventory;
/// an unreadable or malformed one is an error the caller reports as a note.
pub struct PluginInventory;

impl PluginInventory {
    pub fn load(state_dir: &Path) -> Result<Vec<InstalledPluginRecord>, String> {
        let path = state_dir.join("plugins.json");
        let raw = match std::fs::read_to_string(&path) {
            Ok(raw) => raw,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(error) => return Err(format!(".magistr/workspace/plugins.json: {error}")),
        };
        let parsed: serde_json::Value = serde_json::from_str(&raw)
            .map_err(|error| format!(".magistr/workspace/plugins.json: {error}"))?;
        let list = parsed
            .get("plugins")
            .and_then(serde_json::Value::as_array)
            .ok_or_else(|| {
                ".magistr/workspace/plugins.json: `plugins` must be an array".to_owned()
            })?;
        list.iter().map(InstalledPluginRecord::from_value).collect()
    }
}
