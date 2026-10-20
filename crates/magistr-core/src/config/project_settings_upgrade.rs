use crate::{error::Result, types::ProjectConfig};

/// One-way conversion of recognized project settings, without filesystem effects.
/// Callers persist the returned document through their configuration owner.
pub struct ProjectSettingsUpgrade;

impl ProjectSettingsUpgrade {
    pub fn prepare(content: &str) -> Result<Option<String>> {
        let mut document: serde_json::Value = serde_json::from_str(content)?;
        let changed = document["roadmap_backend"] == "think-and-ship";
        if !changed {
            return Ok(None);
        }
        document["roadmap_backend"] = "native".into();
        // Validate the complete result before offering any change. Unknown values
        // remain errors; never replace malformed settings with defaults.
        serde_json::from_value::<ProjectConfig>(document.clone())?;
        Ok(changed.then(|| {
            format!(
                "{}\n",
                serde_json::to_string_pretty(&document).expect("JSON value")
            )
        }))
    }
}
