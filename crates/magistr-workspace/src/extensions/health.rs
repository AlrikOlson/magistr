use std::path::PathBuf;

use super::PluginHealthRecord;

/// The disk-backed circuit breaker, `<state>/plugin-health.json`. Read on
/// every decision (the host holds no in-memory session state); an unreadable
/// or malformed file reads as no breaches, exactly as retained.
pub struct PluginHealth {
    path: PathBuf,
}

impl PluginHealth {
    #[must_use]
    pub fn new(state_dir: &std::path::Path) -> Self {
        Self {
            path: state_dir.join("plugin-health.json"),
        }
    }

    #[must_use]
    pub fn read(&self) -> Vec<PluginHealthRecord> {
        let Ok(raw) = std::fs::read_to_string(&self.path) else {
            return Vec::new();
        };
        let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&raw) else {
            return Vec::new();
        };
        parsed
            .get("breaches")
            .and_then(serde_json::Value::as_array)
            .map(|list| {
                list.iter()
                    .filter_map(|entry| serde_json::from_value(entry.clone()).ok())
                    .collect()
            })
            .unwrap_or_default()
    }

    /// The breach recorded for exactly these component bytes, if any.
    #[must_use]
    pub fn breach(&self, plugin: &str, component_hash: &str) -> Option<PluginHealthRecord> {
        self.read()
            .into_iter()
            .find(|record| record.plugin == plugin && record.component_hash == component_hash)
    }

    pub fn append(&self, record: PluginHealthRecord) -> std::io::Result<()> {
        let mut breaches = self.read();
        breaches.push(record);
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let text = serde_json::to_string_pretty(&serde_json::json!({ "breaches": breaches }))
            .map_err(std::io::Error::other)?;
        std::fs::write(&self.path, format!("{text}\n"))
    }
}
