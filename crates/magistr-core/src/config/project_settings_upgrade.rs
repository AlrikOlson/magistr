use crate::{
    error::{MagistrError, Result},
    types::ProjectConfig,
};
use serde_json::{Map, Value};

/// One-way conversion of recognized project settings, without filesystem effects.
/// Callers persist the returned document through their configuration owner.
pub struct ProjectSettingsUpgrade;

/// Settings keys the plan family named after the roadmap before the
/// vocabulary cutover.
const PLAN_KEYS: [(&str, &str); 2] = [
    ("roadmap_backend", "plan_backend"),
    ("roadmap_exports", "plan_exports"),
];

/// Settings keys that named checks "quality gates" before the check words.
const CHECK_KEYS: [(&str, &str); 2] = [
    ("quality_gate", "checks"),
    ("custom_gates", "custom_checks"),
];

/// Lifecycle events that fired on gates before the check words.
const CHECK_EVENTS: [(&str, &str); 2] = [
    ("on_gate_pass", "on_check_pass"),
    ("on_gate_fail", "on_check_fail"),
];

/// The check tool's name before the check words, and its current name.
const OLDER_CHECK_TOOL: &str = "magistr_gate";
const CHECK_TOOL: &str = "magistr_check";

impl ProjectSettingsUpgrade {
    pub fn prepare(content: &str) -> Result<Option<String>> {
        let mut document: Value = serde_json::from_str(content)?;
        let Some(settings) = document.as_object_mut() else {
            return Ok(None);
        };
        let mut changed = rename_keys(settings, &PLAN_KEYS, "project settings")?;
        if settings.get("plan_backend").and_then(|v| v.as_str()) == Some("think-and-ship") {
            settings.insert("plan_backend".to_owned(), "native".into());
            changed = true;
        }
        changed |= upgrade_check_words(settings)?;
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

/// The check words: the standard and custom check keys, per-check policies
/// under `verification`, the checks guard, each archetype's standard checks,
/// and lifecycle hooks that fire on checks or call the check tool.
fn upgrade_check_words(settings: &mut Map<String, Value>) -> Result<bool> {
    let mut changed = rename_keys(settings, &CHECK_KEYS, "project settings")?;
    for (section, old, new) in [
        ("verification", "gates", "checks"),
        ("guards", "quality_gate", "checks"),
    ] {
        if let Some(object) = settings.get_mut(section).and_then(Value::as_object_mut) {
            changed |= rename_keys(object, &[(old, new)], section)?;
        }
    }
    if let Some(archetypes) = settings.get_mut("archetypes").and_then(Value::as_array_mut) {
        for archetype in archetypes.iter_mut().filter_map(Value::as_object_mut) {
            changed |= rename_keys(archetype, &[CHECK_KEYS[0]], "archetype")?;
        }
    }
    if let Some(hooks) = settings
        .get_mut("lifecycle_hooks")
        .and_then(Value::as_array_mut)
    {
        for hook in hooks.iter_mut().filter_map(Value::as_object_mut) {
            changed |= upgrade_hook(hook)?;
        }
    }
    Ok(changed)
}

fn upgrade_hook(hook: &mut Map<String, Value>) -> Result<bool> {
    let mut changed = false;
    if let Some(kind) = hook
        .get_mut("event")
        .and_then(|event| event.get_mut("type"))
        && let Some((_, new)) = CHECK_EVENTS
            .iter()
            .find(|(old, _)| kind.as_str() == Some(old))
    {
        *kind = Value::String((*new).to_owned());
        changed = true;
    }
    let prefix = hook
        .get("tool")
        .and_then(Value::as_str)
        .and_then(|name| name.strip_suffix(OLDER_CHECK_TOOL))
        .filter(|prefix| prefix.is_empty() || prefix.ends_with("__"))
        .map(str::to_owned);
    if let Some(prefix) = prefix {
        hook.insert(
            "tool".to_owned(),
            Value::String(format!("{prefix}{CHECK_TOOL}")),
        );
        if let Some(arguments) = hook.get_mut("arguments").and_then(Value::as_object_mut) {
            rename_keys(arguments, &[("gate", "check")], "hook arguments")?;
        }
        changed = true;
    }
    Ok(changed)
}

/// Move each present old key to its new name. Holding both spellings is
/// refused: the owner cannot tell which one the person meant.
fn rename_keys(
    object: &mut Map<String, Value>,
    keys: &[(&str, &str)],
    place: &str,
) -> Result<bool> {
    let mut changed = false;
    for (old, new) in keys {
        let Some(value) = object.remove(*old) else {
            continue;
        };
        if object.contains_key(*new) {
            return Err(MagistrError::Config(format!(
                "The {place} hold both {old} and {new}; keep one and retry."
            )));
        }
        object.insert((*new).to_owned(), value);
        changed = true;
    }
    Ok(changed)
}

#[cfg(test)]
#[path = "project_settings_upgrade.tests.rs"]
mod tests;
