//! Config migration — detect and auto-fix legacy configurations.
//!
//! Detects two categories of issues:
//! - **Auto-fixable**: `.jig/` rename, roadmap migration, legacy keys — handled by `run_migration`.
//! - **Manual**: missing `project.json`, bash-era hooks — require `magistr init`.
//!
//! The detection logic (`check_migration`) lives here in magistr-core.
//! The execution logic (`run_migration`) that depends on `orchestration::roadmap`
//! lives in the CLI crate (`src/core/config/migration_exec.rs`).

use std::path::Path;

use crate::error::Result;
use crate::paths;

pub use super::migration_check::{MigrationAction, MigrationCheck};

/// Markers that indicate a Bash-era `.claude/` configuration.
const BASH_SHEBANG: &str = "#!/bin/bash";
const BASH_SHEBANG_ENV: &str = "#!/usr/bin/env bash";

/// Check if a project needs migration. Returns both auto-fixable actions and manual issues.
pub fn check_migration(project_dir: &Path) -> Result<MigrationCheck> {
    let claude_dir = project_dir.join(".claude");
    let mut actions = Vec::new();
    let mut manual_reasons = Vec::new();

    // ── Auto-fixable: .jig/ → .magistr/ ────────────────────────────────
    if paths::has_legacy_jig_dir(project_dir) {
        actions.push(MigrationAction::RenameJigDir);
    }

    // ── Auto-fixable: roadmap_backend missing from project.json ─────────
    // A project that predates the key keeps its YAML roadmap (inferred);
    // writing the resolved value makes the choice explicit and stable.
    let backend = crate::roadmap::backend_for(project_dir);
    if paths::project_json(project_dir).is_file()
        && crate::roadmap::declared_backend(project_dir).is_none()
    {
        actions.push(MigrationAction::WriteRoadmapBackend(backend));
    }

    // ── Auto-fixable: roadmap file migration (yaml projects only) ───────
    // On a think-and-ship project ROADMAP.md is a generated view, not a
    // source to migrate.
    let yaml_path = paths::roadmap_yaml(project_dir);
    if !backend.is_native() && !yaml_path.exists() {
        let legacy_yaml = paths::legacy_roadmap_yaml(project_dir);
        if legacy_yaml.exists() {
            actions.push(MigrationAction::MigrateRoadmapYaml);
        } else if project_dir.join("ROADMAP.md").exists() {
            actions.push(MigrationAction::MigrateRoadmapMd);
        }
    }

    // ── Auto-fixable: jig_version in settings.json ──────────────────────
    if claude_dir.is_dir() {
        check_legacy_settings(&claude_dir, &mut actions);
    }

    // ── Manual: missing project.json ────────────────────────────────────
    // Check in .magistr/ (or .jig/ if rename hasn't happened yet)
    let has_project_json = project_dir.join(".magistr").join("project.json").is_file()
        || project_dir.join(".jig").join("project.json").is_file();

    if claude_dir.is_dir() && !has_project_json {
        manual_reasons.push("Missing .magistr/project.json (required by Magistr)".to_string());
    }

    // ── Manual: bash-era hooks ──────────────────────────────────────────
    if claude_dir.is_dir() {
        check_bash_hooks(&claude_dir, &mut manual_reasons);
    }

    Ok(MigrationCheck {
        actions,
        manual_reasons,
        claude_dir,
    })
}

/// Execute simple (non-roadmap) migration actions. Returns descriptions of what was done.
///
/// Handles `RenameJigDir`, `RemoveJigVersionKey`, and `spec.file` patching.
/// Roadmap migrations (`MigrateRoadmapYaml`, `MigrateRoadmapMd`) are skipped —
/// those require `orchestration::roadmap` and must be handled by the CLI crate's
/// `run_migration` function.
pub fn run_simple_migrations(project_dir: &Path, check: &MigrationCheck) -> Result<Vec<String>> {
    use crate::error::MagistrError;
    let mut completed = Vec::new();

    for action in &check.actions {
        match action {
            MigrationAction::RenameJigDir => {
                paths::migrate_jig_dir(project_dir)
                    .map_err(|e| MagistrError::io(project_dir.join(".jig"), e))?;
                completed.push(action.description().to_string());
            }
            MigrationAction::RemoveJigVersionKey => {
                remove_jig_version_key(project_dir);
                completed.push(action.description().to_string());
            }
            MigrationAction::WriteRoadmapBackend(backend) => {
                write_roadmap_backend(project_dir, *backend)?;
                completed.push(action.description().to_string());
            }
            // Roadmap migrations require orchestration::roadmap — handled by CLI crate
            MigrationAction::MigrateRoadmapYaml | MigrationAction::MigrateRoadmapMd => {}
        }
    }

    // Patch project.json spec.file if needed
    patch_project_json_spec_file(project_dir);

    Ok(completed)
}

/// Write `"roadmap_backend": "<backend>"` into `.magistr/project.json`,
/// leaving every other key as it is.
pub fn write_roadmap_backend(
    project_dir: &Path,
    backend: magistr_engine::native::RoadmapBackend,
) -> Result<()> {
    use crate::error::MagistrError;
    let config_path = paths::project_json(project_dir);
    let raw =
        std::fs::read_to_string(&config_path).map_err(|e| MagistrError::io(&config_path, e))?;
    let mut value: serde_json::Value =
        serde_json::from_str(&raw).map_err(|e| MagistrError::Config(e.to_string()))?;
    let Some(object) = value.as_object_mut() else {
        return Err(MagistrError::Config(
            "project.json is not a JSON object".to_owned(),
        ));
    };
    object.insert(
        "roadmap_backend".to_owned(),
        serde_json::Value::String(backend.as_str().to_owned()),
    );
    let pretty =
        serde_json::to_string_pretty(&value).map_err(|e| MagistrError::Config(e.to_string()))?;
    std::fs::write(&config_path, format!("{pretty}\n"))
        .map_err(|e| MagistrError::io(&config_path, e))
}

/// Check hook scripts for bash shebangs indicating Bash-era generation.
fn check_bash_hooks(claude_dir: &Path, reasons: &mut Vec<String>) {
    let hooks_dir = claude_dir.join("hooks");
    if !hooks_dir.is_dir() {
        return;
    }

    let Ok(entries) = std::fs::read_dir(&hooks_dir) else {
        return;
    };

    for entry in entries.filter_map(|e| e.ok()) {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }

        let Ok(content) = std::fs::read_to_string(&path) else {
            continue;
        };

        let first_line = content.lines().next().unwrap_or("");
        if (first_line.starts_with(BASH_SHEBANG) || first_line.starts_with(BASH_SHEBANG_ENV))
            && let Some(name) = path.file_name().and_then(|n| n.to_str())
        {
            reasons.push(format!(
                "Hook script '{name}' uses bash shebang (Bash-era format)"
            ));
        }
    }
}

/// Check for legacy `jig_version` key in settings.json.
fn check_legacy_settings(claude_dir: &Path, actions: &mut Vec<MigrationAction>) {
    let settings_path = claude_dir.join("settings.json");
    if !settings_path.is_file() {
        return;
    }

    let Ok(content) = std::fs::read_to_string(&settings_path) else {
        return;
    };

    if content.contains("\"jig_version\"") {
        actions.push(MigrationAction::RemoveJigVersionKey);
    }
}

/// Remove the `jig_version` key from `.claude/settings.json`.
fn remove_jig_version_key(project_dir: &Path) {
    let settings_path = project_dir.join(".claude").join("settings.json");
    let Ok(content) = std::fs::read_to_string(&settings_path) else {
        return;
    };
    let Ok(mut json) = serde_json::from_str::<serde_json::Value>(&content) else {
        return;
    };
    if let Some(obj) = json.as_object_mut() {
        obj.remove("jig_version");
        if let Ok(pretty) = serde_json::to_string_pretty(&json) {
            let _ = std::fs::write(&settings_path, pretty + "\n");
        }
    }
}

/// Patch `spec.file` in `.magistr/project.json` from `"ROADMAP.md"` to `".magistr-roadmap.yml"`.
fn patch_project_json_spec_file(project_dir: &Path) {
    let config_path = project_dir.join(".magistr").join("project.json");
    if !config_path.exists() {
        return;
    }
    let Ok(content) = std::fs::read_to_string(&config_path) else {
        return;
    };
    if !content.contains("\"ROADMAP.md\"") {
        return;
    }
    let patched = content.replace("\"ROADMAP.md\"", "\".magistr-roadmap.yml\"");
    let _ = std::fs::write(&config_path, patched);
}

#[cfg(test)]
#[path = "migration.tests.rs"]
mod tests;
