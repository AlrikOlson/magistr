//! Drift between `.magistr/project.json`'s gates and `workbench.toml
//! [commands]` — the allowlist workbench `run` executes from. `magistr init`
//! seeds the table once and never touches it again, so a later edit to
//! either side silently desynchronises the two; `magistr lint` and
//! `magistr doctor` surface that.

use std::path::Path;

use magistr_core::detection::lint::{CheckStatus, DoctorCheck};

use super::WorkbenchSeed;

/// The drift findings for `dir`, or `None` when there is nothing to compare
/// (no `workbench.toml`, or no readable `.magistr/project.json`). An empty
/// list means both files agree.
#[must_use]
pub(crate) fn workbench_drift(dir: &Path) -> Option<Vec<String>> {
    let body = std::fs::read_to_string(dir.join("workbench.toml")).ok()?;
    let config = magistr_core::validate::load_project_config(dir).ok()?;
    Some(WorkbenchSeed::from_project_config(&config).drift(&body))
}

/// The `magistr doctor` row for the comparison.
#[must_use]
pub(crate) fn doctor_check(dir: &Path) -> DoctorCheck {
    let description = "workbench.toml [commands] matches the project.json gates".to_owned();
    match workbench_drift(dir) {
        None if dir.join("workbench.toml").is_file() => DoctorCheck {
            name: "workbench-commands",
            description,
            status: CheckStatus::Warn,
            detail: Some("could not read .magistr/project.json to compare against".to_owned()),
        },
        None => DoctorCheck {
            name: "workbench-commands",
            description,
            status: CheckStatus::Warn,
            detail: Some(
                "no workbench.toml — run `magistr init` to seed workbench run's allowlist"
                    .to_owned(),
            ),
        },
        Some(findings) if findings.is_empty() => DoctorCheck {
            name: "workbench-commands",
            description,
            status: CheckStatus::Pass,
            detail: None,
        },
        Some(findings) => DoctorCheck {
            name: "workbench-commands",
            description,
            status: CheckStatus::Warn,
            detail: Some(findings.join("; ")),
        },
    }
}

#[cfg(test)]
#[path = "workbench_drift.tests.rs"]
mod tests;
