//! Drift between `.magistr/project.json`'s gates and `.magistr/workspace.toml
//! [commands]` — the allowlist workspace `run` executes from. `magistr init`
//! seeds the table once and never touches it again, so a later edit to
//! either side silently desynchronises the two; `magistr lint` and
//! `magistr doctor` surface that.

use std::path::Path;

use magistr_core::detection::lint::{CheckStatus, DoctorCheck};

use super::WorkspaceSeed;

/// The drift findings for `dir`, or `None` when there is nothing to compare
/// (no `.magistr/workspace.toml`, or no readable `.magistr/project.json`). An empty
/// list means both files agree.
#[must_use]
pub(crate) fn workspace_drift(dir: &Path) -> Option<Vec<String>> {
    let body = std::fs::read_to_string(dir.join(".magistr/workspace.toml")).ok()?;
    let config = magistr_core::validate::load_project_config(dir).ok()?;
    Some(WorkspaceSeed::from_project_config(&config).drift(&body))
}

/// The `magistr doctor` row for the comparison.
#[must_use]
pub(crate) fn doctor_check(dir: &Path) -> DoctorCheck {
    let description =
        ".magistr/workspace.toml [commands] matches the project.json gates".to_owned();
    match workspace_drift(dir) {
        None if dir.join(".magistr/workspace.toml").is_file() => DoctorCheck {
            name: "workspace-commands",
            description,
            status: CheckStatus::Warn,
            detail: Some("could not read .magistr/project.json to compare against".to_owned()),
        },
        None => DoctorCheck {
            name: "workspace-commands",
            description,
            status: CheckStatus::Warn,
            detail: Some(
                "no .magistr/workspace.toml — run `magistr init` to seed workspace run's allowlist"
                    .to_owned(),
            ),
        },
        Some(findings) if findings.is_empty() => DoctorCheck {
            name: "workspace-commands",
            description,
            status: CheckStatus::Pass,
            detail: None,
        },
        Some(findings) => DoctorCheck {
            name: "workspace-commands",
            description,
            status: CheckStatus::Warn,
            detail: Some(findings.join("; ")),
        },
    }
}

#[cfg(test)]
#[path = "workspace_drift.tests.rs"]
mod tests;
