//! Save one agent preference while preserving other settings and TOML comments.

use crate::config::agent_preferences::AgentPreferences;
use crate::config::inputs::ProjectInputs;
use color_eyre::eyre::{self, WrapErr};
use std::path::Path;
use toml_edit::{DocumentMut, value};

pub(super) fn config_agent_set(
    key: &str,
    setting: &str,
    project: bool,
    agent: Option<&str>,
    dir: &Path,
    inputs: &ProjectInputs,
) -> eyre::Result<()> {
    if !["agent", "model", "effort"].contains(&key) {
        eyre::bail!("Choose a setting: agent, model, or effort");
    }
    if setting.trim().is_empty() || setting.trim() != setting {
        eyre::bail!("Enter a value without surrounding spaces");
    }
    let inputs = inputs.for_project(dir);
    let path = if project {
        dir.join(".magistr/agents.toml")
    } else {
        inputs
            .config_dir()
            .ok_or_else(|| {
                eyre::eyre!(
                    "Personal settings directory is unavailable; set HOME or XDG_CONFIG_HOME"
                )
            })?
            .join("config.toml")
    };
    let original = match std::fs::read_to_string(&path) {
        Ok(text) => text,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(error) => {
            return Err(error).wrap_err_with(|| format!("Cannot read {}", path.display()));
        }
    };
    if !original.is_empty() {
        if project {
            toml::from_str::<AgentPreferences>(&original)?.validate()?;
        } else {
            let _: crate::config::user_config::UserConfig = toml::from_str(&original)?;
        }
    }
    let mut doc: DocumentMut = original
        .parse()
        .wrap_err_with(|| format!("Cannot read settings in {}", path.display()))?;
    if key == "agent" {
        if agent.is_some() {
            eyre::bail!(
                "Use --agent when saving a model or effort; the agent setting already names an agent"
            );
        }
        validate_agent(setting, true)?;
        if project {
            doc["agent"] = value(setting);
        } else {
            doc["session"]["default_provider"] = value(setting);
        }
    } else {
        let personal = inputs.user_config()?.value;
        let prefs = if project {
            AgentPreferences::load(dir)?
        } else {
            AgentPreferences::default()
        };
        let selected = agent
            .or(if project {
                prefs.agent.as_deref()
            } else {
                None
            })
            .or(personal.session.provider())
            .ok_or_else(|| {
                eyre::eyre!(
                    "Choose an agent first, or supply --agent when saving a model or effort"
                )
            })?;
        validate_agent(selected, false)?;
        doc[if key == "model" { "models" } else { "efforts" }][selected] = value(setting);
    }
    let updated = doc.to_string();
    if project {
        toml::from_str::<AgentPreferences>(&updated)?.validate()?;
    } else {
        let _: crate::config::user_config::UserConfig = toml::from_str(&updated)?;
    }
    crate::workflow::durable_write::durable_write(&path, updated.as_bytes())?;
    eprintln!("Saved {key} in {}", path.display());
    Ok(())
}

fn validate_agent(agent: &str, auto: bool) -> eyre::Result<()> {
    if ["claude", "copilot", "codex", "openai", "gemini", "ollama"].contains(&agent)
        || (auto && agent == "auto")
    {
        return Ok(());
    }
    eyre::bail!(
        "Choose a named agent: claude, copilot, codex, openai, gemini, or ollama{}",
        if auto { ", or auto" } else { "" }
    )
}

#[cfg(test)]
#[path = "config_agent_set.tests.rs"]
mod tests;
