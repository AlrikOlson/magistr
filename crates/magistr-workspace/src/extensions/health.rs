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

    /// Record one breach at `at`. A breach that cannot be persisted is
    /// returned as the note every lane appends: the breaker then will not
    /// short-circuit, and the envelope says so.
    #[must_use]
    pub fn record(
        &self,
        plugin: &str,
        component_hash: &str,
        reason: &str,
        at: &str,
    ) -> Option<String> {
        self.append(PluginHealthRecord {
            plugin: plugin.to_owned(),
            component_hash: component_hash.to_owned(),
            reason: reason.to_owned(),
            at: at.to_owned(),
        })
        .err()
        .map(|error| {
            format!(
                "plugin {plugin}: breach could not be recorded ({error}); the breaker will not short-circuit"
            )
        })
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
