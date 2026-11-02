use crate::{error::Result, types::ProjectConfig};

/// One-way conversion of recognized project settings, without filesystem effects.
/// Callers persist the returned document through their configuration owner.
pub struct ProjectSettingsUpgrade;

/// Settings keys the plan family named after the roadmap before the
/// vocabulary cutover.
const PLAN_KEYS: [(&str, &str); 2] = [
    ("roadmap_backend", "plan_backend"),
    ("roadmap_exports", "plan_exports"),
];

impl ProjectSettingsUpgrade {
    pub fn prepare(content: &str) -> Result<Option<String>> {
        let mut document: serde_json::Value = serde_json::from_str(content)?;
        let Some(settings) = document.as_object_mut() else {
            return Ok(None);
        };
        let mut changed = false;
        for (old, new) in PLAN_KEYS {
            let Some(value) = settings.remove(old) else {
                continue;
            };
            if settings.contains_key(new) {
                return Err(crate::error::MagistrError::Config(format!(
                    "The project settings hold both {old} and {new}; keep one and retry."
                )));
            }
            settings.insert(new.to_owned(), value);
            changed = true;
        }
        if settings.get("plan_backend").and_then(|v| v.as_str()) == Some("think-and-ship") {
            settings.insert("plan_backend".to_owned(), "native".into());
            changed = true;
        }
        if !changed {
            return Ok(None);
        }
        // Validate the complete result before offering any change. Unknown values
        // remain errors; never replace malformed settings with defaults.
        serde_json::from_value::<ProjectConfig>(document.clone())?;
        Ok(Some(format!(
            "{}\n",
            serde_json::to_string_pretty(&document).expect("JSON value")
        )))
    }
}

#[cfg(test)]
#[path = "project_settings_upgrade.tests.rs"]
mod tests;
