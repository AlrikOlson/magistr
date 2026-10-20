//! Opening intent selects one conversation through the existing admission owner.
use crate::agent::AgentConfig;
use serde::Deserialize;

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum SessionOpening {
    #[default]
    Launch,
    New {
        provider: String,
        model: Option<String>,
    },
    Resume {
        id: String,
    },
}
impl SessionOpening {
    pub(super) fn apply(self, mut config: AgentConfig) -> crate::error::Result<AgentConfig> {
        match self {
            Self::Launch => Ok(config),
            Self::New { provider, model } => {
                config.prompt.clear();
                config.model = model;
                config.effort = None;
                config.resume = None;
                config.history = crate::agent::persistence::SessionHistory::new();
                Ok(config.provider(provider))
            }
            Self::Resume { id } => {
                config.prompt.clear();
                config.provider = None;
                config.resume = None;
                config.with_run_args(None, Some(&id), false)
            }
        }
    }
}
#[cfg(test)]
#[path = "session_opening.tests.rs"]
mod tests;
