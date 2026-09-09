//! Custom role loader — reads agent role definitions from `.magistr/agents/*.toml`.
//!
//! Merges custom roles with built-in archetypes. Custom roles with the same
//! `role_id` as a built-in override the built-in definition.

use std::path::Path;

use serde::Deserialize;
use tracing::warn;

use super::role::{RoleConstraints, SpecializedRole};

// ─── TOML Schema ──────────────────────────────────────────────────────────

/// On-disk TOML representation of a custom agent role.
#[derive(Debug, Deserialize)]
struct RoleToml {
    role_id: String,
    display_name: String,
    system_prompt_template: String,
    #[serde(default)]
    allowed_tools: Vec<String>,
    #[serde(default)]
    denied_tools: Vec<String>,
    #[serde(default = "default_icon_str")]
    icon: String,
    #[serde(default)]
    color_index: u8,
    #[serde(default)]
    constraints: ConstraintsToml,
    #[serde(default)]
    model: ModelToml,
}

#[derive(Debug, Default, Deserialize)]
struct ConstraintsToml {
    #[serde(default)]
    read_only: bool,
    #[serde(default)]
    max_iterations: u32,
}

#[derive(Debug, Default, Deserialize)]
struct ModelToml {
    preference: Option<String>,
    #[serde(default)]
    fallback: Vec<String>,
}

fn default_icon_str() -> String {
    "\u{2699}".into() // ⚙
}

impl From<RoleToml> for SpecializedRole {
    fn from(toml: RoleToml) -> Self {
        let icon = toml.icon.chars().next().unwrap_or('\u{2699}');
        Self {
            role_id: toml.role_id,
            display_name: toml.display_name,
            system_prompt_template: toml.system_prompt_template,
            allowed_tools: toml.allowed_tools,
            denied_tools: toml.denied_tools,
            model_preference: toml.model.preference,
            model_fallback: toml.model.fallback,
            constraints: RoleConstraints {
                read_only: toml.constraints.read_only,
                max_iterations: toml.constraints.max_iterations,
            },
            icon,
            color_index: toml.color_index,
            archetype_id: None,
        }
    }
}

// ─── Loading ──────────────────────────────────────────────────────────────

/// Load custom roles from `.magistr/agents/*.toml` in the given project directory.
///
/// Invalid TOML files are skipped with a warning. Non-TOML files are ignored.
pub fn load_custom_roles(project_dir: &Path) -> Vec<SpecializedRole> {
    let agents_dir = project_dir.join(".magistr").join("agents");
    let entries = match std::fs::read_dir(&agents_dir) {
        Ok(entries) => entries,
        Err(_) => return Vec::new(),
    };

    let mut roles = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("toml") {
            continue;
        }

        let content = match std::fs::read_to_string(&path) {
            Ok(c) => c,
            Err(e) => {
                warn!("Failed to read {}: {e}", path.display());
                continue;
            }
        };

        match toml::from_str::<RoleToml>(&content) {
            Ok(toml_role) => roles.push(SpecializedRole::from(toml_role)),
            Err(e) => {
                warn!("Failed to parse {}: {e}", path.display());
            }
        }
    }

    roles
}

/// Load all roles: built-in archetypes merged with custom roles from disk.
///
/// Custom roles with the same `role_id` as a built-in override the built-in.
pub fn load_all_roles(project_dir: &Path) -> Vec<SpecializedRole> {
    let mut roles = SpecializedRole::built_in_roles();
    let custom = load_custom_roles(project_dir);

    for custom_role in custom {
        if let Some(existing) = roles.iter_mut().find(|r| r.role_id == custom_role.role_id) {
            *existing = custom_role;
        } else {
            roles.push(custom_role);
        }
    }

    roles
}

#[cfg(test)]
#[path = "role_loader.tests.rs"]
mod tests;
