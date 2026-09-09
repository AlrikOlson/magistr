//! `magistr init --check` — drift detection between filesystem and config.

use crate::detect;
use crate::error::{self, ResultExt};
use crate::scaffold;
use crate::utils::format as fmt;
use crate::validate;
use color_eyre::eyre;

pub fn cmd_init_check(dir: &std::path::Path) -> eyre::Result<()> {
    let dir = dir
        .canonicalize()
        .map_err(|e| error::MagistrError::io(dir, e))
        .suggest()?;

    // Load existing config
    let config = validate::load_project_config(&dir)?;
    let archetypes = config.effective_archetypes();

    // Re-detect the project tree
    eprintln!(
        "{}",
        fmt::info(&format!("Scanning {} for drift...", dir.display()))
    );
    let tree = detect::build_project_tree(&dir).suggest()?;

    let report = scaffold::drift::detect_drift(&archetypes, &tree);

    if report.is_clean() {
        eprintln!(
            "{}",
            fmt::success("No drift detected — config matches filesystem")
        );
        return Ok(());
    }

    eprintln!(
        "{}",
        fmt::warning(&format!(
            "Detected {} change(s) between config and filesystem:",
            report.total()
        ))
    );
    eprintln!();

    if !report.new_projects.is_empty() {
        eprintln!("{}", fmt::bold("New projects detected:"));
        for project in &report.new_projects {
            let path = if project.path.as_os_str().is_empty() {
                ".".to_string()
            } else {
                project.path.display().to_string()
            };
            let lang = project.language.as_str();
            let fw = project
                .framework
                .map(|f| format!(" + {}", f.as_str()))
                .unwrap_or_default();
            eprintln!("  {} {path} [{lang}{fw}]", fmt::green("+"));
        }
        eprintln!();
    }

    if !report.removed_archetypes.is_empty() {
        eprintln!("{}", fmt::bold("Removed archetypes:"));
        for id in &report.removed_archetypes {
            eprintln!("  {} {id}", fmt::red("-"));
        }
        eprintln!();
    }

    if !report.changes.is_empty() {
        eprintln!("{}", fmt::bold("Changes:"));
        for change in &report.changes {
            eprintln!(
                "  {} [{}] {}",
                fmt::yellow("~"),
                change.archetype_id,
                change.description
            );
        }
        eprintln!();
    }

    eprintln!(
        "Run {} to apply detected changes.",
        fmt::bold("magistr init --force")
    );

    Ok(())
}
