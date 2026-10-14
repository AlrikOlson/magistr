//! Save one agent preference through the owning settings writer.

use crate::config::agent_preferences::AgentPreferences;
use crate::config::inputs::ProjectInputs;
use crate::config::user_config::{UserConfig, UserConfigDocument};
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
    if !project {
        let path = inputs.user_config_path().ok_or_else(|| {
            eyre::eyre!("Personal settings directory is unavailable; set HOME or XDG_CONFIG_HOME")
        })?;
        let document = UserConfigDocument::load(&path)?;
        let (section, property) = setting_target(key, setting, agent, None, document.config())?;
        document.save(&[(section, &property, setting.into())])?;
        eprintln!("Saved {key} in {}", path.display());
        return Ok(());
    }

    let path = dir.join(".magistr/agents.toml");
    let original = match std::fs::read_to_string(&path) {
        Ok(text) => text,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(error) => {
            return Err(error).wrap_err_with(|| format!("Cannot read {}", path.display()));
        }
    };
    let prefs: AgentPreferences = toml::from_str(&original)?;
    prefs.validate()?;
    let mut doc: DocumentMut = original
        .parse()
        .wrap_err_with(|| format!("Cannot read settings in {}", path.display()))?;
    let personal = if key == "agent" {
        UserConfig::default()
    } else {
        inputs.user_config()?.value
    };
    let (section, property) = setting_target(key, setting, agent, Some(&prefs), &personal)?;
    if key == "agent" {
        doc["agent"] = value(setting);
    } else {
        doc[section][&property] = value(setting);
    }
    let updated = doc.to_string();
    toml::from_str::<AgentPreferences>(&updated)?.validate()?;
    magistr_core::durable_write::durable_write(&path, updated.as_bytes())?;
    eprintln!("Saved {key} in {}", path.display());
    Ok(())
}

fn setting_target(
    key: &str,
    setting: &str,
    agent: Option<&str>,
    project: Option<&AgentPreferences>,
    personal: &UserConfig,
) -> eyre::Result<(&'static str, String)> {
    if key == "agent" {
        if agent.is_some() {
            eyre::bail!(
                "Use --agent when saving a model or effort; the agent setting already names an agent"
            );
        }
        validate_agent(setting, true)?;
        return Ok(("session", "default_provider".into()));
    }
    let selected = agent
        .or_else(|| project.and_then(|prefs| prefs.agent.as_deref()))
        .or(personal.session.provider())
        .ok_or_else(|| {
            eyre::eyre!("Choose an agent first, or supply --agent when saving a model or effort")
        })?;
    validate_agent(selected, false)?;
    Ok((
        if key == "model" { "models" } else { "efforts" },
        selected.into(),
    ))
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
