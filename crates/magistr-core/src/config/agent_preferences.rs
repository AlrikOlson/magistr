//! Project defaults for agent selection; models and effort are scoped to the agent.

use crate::error::{MagistrError, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::Path;

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AgentPreferences {
    pub agent: Option<String>,
    #[serde(default)]
    pub models: BTreeMap<String, String>,
    #[serde(default)]
    pub efforts: BTreeMap<String, String>,
}

impl AgentPreferences {
    pub fn load(root: &Path) -> Result<Self> {
        let path = root.join(".magistr/agents.toml");
        let text = match std::fs::read_to_string(&path) {
            Ok(text) => text,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Ok(Self::default());
            }
            Err(error) => return Err(MagistrError::io(&path, error)),
        };
        let prefs: Self = toml::from_str(&text)
            .map_err(|error| MagistrError::parse("TOML", path.display().to_string(), error))?;
        prefs.validate()?;
        Ok(prefs)
    }

    pub fn validate(&self) -> Result<()> {
        for value in self
            .agent
            .iter()
            .chain(self.models.values())
            .chain(self.efforts.values())
        {
            if value.trim().is_empty() || value.trim() != value {
                return Err(MagistrError::Config(
                    "Agent settings must contain a value without surrounding spaces".into(),
                ));
            }
        }
        Ok(())
    }
}

#[cfg(test)]
#[path = "agent_preferences.tests.rs"]
mod tests;
