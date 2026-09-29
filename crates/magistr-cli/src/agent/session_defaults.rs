//! Agent-scoped defaults and frozen selections used by every native startup path.
use crate::agent::{AgentConfig, AgentRuntime, SessionConfigSource};
use crate::config::user_config::UserConfig;
use crate::error::Result;
use std::collections::BTreeMap;

pub(super) fn session_defaults(
    config: &mut AgentConfig,
    defaults: &UserConfig,
    runtime: &AgentRuntime,
) -> Result<BTreeMap<&'static str, SessionConfigSource>> {
    use SessionConfigSource::{
        AutoDetected, Explicit, ProjectFile, RuntimeDefault, RuntimeResume, UserDefault,
    };
    let mut provenance = BTreeMap::new();
    let explicit_effort = config.effort.is_some();
    let snapshot = config
        .inputs
        .session_selection()
        .filter(|selection| selection.agent == runtime.provider)
        .cloned();
    let project = if snapshot.is_some() {
        crate::config::agent_preferences::AgentPreferences::default()
    } else {
        crate::config::agent_preferences::AgentPreferences::load(&config.cwd)?
    };
    let provider_source = match config.provider.as_deref() {
        Some("auto") => AutoDetected,
        Some(_) => Explicit,
        None if snapshot.is_some() => Explicit,
        None if project.agent.is_some() => ProjectFile,
        None if defaults.session.provider().is_some() => UserDefault,
        None => AutoDetected,
    };
    provenance.insert("provider", provider_source);
    config.provider = Some(runtime.provider.clone());
    let model_source = if config.model.is_some() {
        Explicit
    } else if let Some(snapshot) = &snapshot {
        config.model.clone_from(&snapshot.model);
        Explicit
    } else if config.resume.is_none()
        && let Some(model) = project.models.get(runtime.provider_id())
    {
        config.model = Some(model.clone());
        ProjectFile
    } else if config.resume.is_none()
        && let Some(model) = defaults.models.for_provider(runtime.provider_id())
    {
        config.model = Some(model.to_owned());
        UserDefault
    } else if config.resume.is_some() {
        RuntimeResume
    } else {
        RuntimeDefault
    };
    // "default" is a request to retain the provider's selected model.
    config.model = config
        .model
        .take()
        .filter(|model| !model.eq_ignore_ascii_case("default"));
    provenance.insert("model", model_source);
    if config.effort.is_none() {
        if let Some(snapshot) = &snapshot {
            config.effort.clone_from(&snapshot.effort);
            provenance.insert("effort", Explicit);
        } else if config.resume.is_none() {
            if let Some(effort) = project.efforts.get(runtime.provider_id()) {
                config.effort = Some(effort.clone());
                provenance.insert("effort", ProjectFile);
            } else if let Some(effort) = defaults.efforts.get(runtime.provider_id()) {
                config.effort = Some(effort.clone());
                provenance.insert("effort", UserDefault);
            }
        }
    }
    config.effort = config
        .effort
        .take()
        .filter(|effort| !effort.eq_ignore_ascii_case("default"));
    provenance.entry("effort").or_insert(if explicit_effort {
        Explicit
    } else if config.resume.is_some() {
        RuntimeResume
    } else {
        RuntimeDefault
    });
    Ok(provenance)
}

#[cfg(test)]
#[path = "session_defaults.tests.rs"]
mod tests;
