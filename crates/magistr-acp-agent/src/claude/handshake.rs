//! What the `initialize` handshake answers, read for the picker and the
//! command list.

use agent_client_protocol::schema::v1::{
    AvailableCommand, AvailableCommandInput, UnstructuredCommandInput,
};
use serde::Deserialize;
use serde_json::Value;

use crate::ModelChoice;

/// The parts of the initialize response the agent advertises onward: the
/// model catalog (as the picker) and the slash commands (as
/// `available_commands_update`, so a client finds `goal` by name).
#[derive(Debug, Clone, PartialEq, Default)]
pub(crate) struct Handshake {
    /// The models, in the CLI's order — the first is its default.
    pub models: Vec<ModelChoice>,
    /// The slash commands the session accepts as `/name …` prompts.
    pub commands: Vec<AvailableCommand>,
}

#[derive(Deserialize)]
struct ModelInfo {
    value: String,
    #[serde(rename = "displayName", default)]
    display_name: String,
}

#[derive(Deserialize)]
struct SlashCommand {
    name: String,
    #[serde(default)]
    description: String,
    #[serde(rename = "argumentHint", default)]
    argument_hint: String,
}

impl Handshake {
    /// Read the initialize response's payload. Missing lists are empty.
    pub fn parse(payload: &Value) -> Self {
        let models: Vec<ModelInfo> =
            serde_json::from_value(payload["models"].clone()).unwrap_or_default();
        let commands: Vec<SlashCommand> =
            serde_json::from_value(payload["commands"].clone()).unwrap_or_default();
        Self {
            models: models
                .into_iter()
                .map(|m| {
                    let label = if m.display_name.is_empty() {
                        m.value.clone()
                    } else {
                        m.display_name
                    };
                    ModelChoice::new(m.value, label)
                })
                .collect(),
            commands: commands
                .into_iter()
                .map(|c| {
                    let command = AvailableCommand::new(c.name, c.description);
                    if c.argument_hint.is_empty() {
                        command
                    } else {
                        command.input(AvailableCommandInput::Unstructured(
                            UnstructuredCommandInput::new(c.argument_hint),
                        ))
                    }
                })
                .collect(),
        }
    }
}

#[cfg(test)]
#[path = "handshake.tests.rs"]
mod tests;
