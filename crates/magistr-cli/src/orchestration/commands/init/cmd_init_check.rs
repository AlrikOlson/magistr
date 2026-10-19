//! `magistr init --check` — the read-only report of what magistr would
//! change: the drift between the configured archetypes and the tree, and
//! the changes an explicit `magistr repair` would apply.

use super::{PlanDetail, check_report_json, print_plan_plain};
use crate::detect;
use crate::error::{self, ResultExt};
use crate::output::OutputRenderer;
use crate::scaffold;
use crate::utils::format as fmt;
use crate::validate;
use color_eyre::eyre;

/// Report drift and the pending repair without writing. With a
/// machine-readable renderer the whole report is one JSON object on
/// stdout; otherwise it is printed on stderr, the repair plan with its
/// diffs and every refused path with its reason.
pub fn cmd_init_check(dir: &std::path::Path, renderer: &dyn OutputRenderer) -> eyre::Result<()> {
    let dir = dir
        .canonicalize()
        .map_err(|e| error::MagistrError::io(dir, e))
        .suggest()?;

    // Load existing config
    let config = validate::load_project_config(&dir)?;
    let archetypes = config.effective_archetypes();

    if !renderer.is_machine_readable() {
        eprintln!(
            "{}",
            fmt::info(&format!("Scanning {} for drift...", dir.display()))
        );
    }
    let tree = detect::build_project_tree(&dir).suggest()?;
    let report = scaffold::drift::detect_drift(&archetypes, &tree);
    let repair = scaffold::repair::plan(&dir).suggest()?;

    if renderer.is_machine_readable() {
        let json = check_report_json(&report, &repair, PlanDetail::Full);
        println!("{}", serde_json::to_string_pretty(&json)?);
        return Ok(());
    }

    print_drift_plain(&report);
    eprintln!();
    print_repair_plain(&repair);
    Ok(())
}

fn print_drift_plain(report: &scaffold::drift::DriftReport) {
    if report.is_clean() {
        eprintln!(
            "{}",
            fmt::success("No drift detected — config matches filesystem")
        );
        return;
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
}

/// The repair plan: what the explicit `magistr repair` writes to
/// the files magistr owns outright, and what it will not touch and why.
fn print_repair_plain(repair: &scaffold::ChangePlan) {
    if repair.files.is_empty() {
        eprintln!(
            "{}",
            fmt::success("Repair: nothing pending — owned files are at this magistr's version")
        );
        return;
    }
    print_plan_plain(
        repair,
        "🔧 Repair (planned — applied at the explicit `magistr repair`, nothing is written now)",
        Some(&format!(
            "Run {} to review and apply the repair; refused paths are left untouched.",
            fmt::bold("magistr repair")
        )),
    );
}

#[cfg(test)]
#[path = "cmd_init_check.tests.rs"]
mod tests;
