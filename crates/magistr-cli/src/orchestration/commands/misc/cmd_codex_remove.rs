//! `magistr codex-remove` — the owned removal of the managed Codex target,
//! and its read-only dry run.

use crate::commands::init::{PlanDetail, plan_json, print_plan_plain};
use crate::error::{self, ResultExt};
use crate::output::OutputRenderer;
use crate::scaffold::CodexSetup;
use crate::utils::format as fmt;
use color_eyre::eyre;

/// Remove the managed Codex setup from `dir`, or with `dry_run` print the
/// removal plan the removal would follow — every path with its action,
/// ownership and reason — without writing. A machine-readable renderer
/// gets the plan (or the removed paths) as JSON on stdout.
pub fn cmd_codex_remove(
    dir: &std::path::Path,
    dry_run: bool,
    renderer: &dyn OutputRenderer,
) -> eyre::Result<()> {
    let dir = dir
        .canonicalize()
        .map_err(|e| error::MagistrError::io(dir, e))
        .suggest()?;

    if dry_run {
        // Planned only: `remove_target` computes this same plan and applies
        // it through the administrative service; nothing is written here.
        let plan = CodexSetup::removal_plan(&dir).suggest()?;
        if renderer.is_machine_readable() {
            let json = plan_json(&plan, PlanDetail::Full);
            println!("{}", serde_json::to_string_pretty(&json)?);
            return Ok(());
        }
        if plan.files.is_empty() {
            eprintln!(
                "{}",
                fmt::info(&format!(
                    "No managed Codex setup is recorded in {}; nothing to remove.",
                    dir.display()
                ))
            );
            return Ok(());
        }
        print_plan_plain(
            &plan,
            "🗑  Codex removal preview (dry run — no files will be written)",
            Some(&format!(
                "Run {} to apply these changes; a preserved or refused path stays as it is.",
                fmt::bold("magistr codex-remove")
            )),
        );
        return Ok(());
    }

    let paths = CodexSetup::remove_target(&dir).suggest()?;
    if renderer.is_machine_readable() {
        let json = serde_json::json!({
            "operation": "removal",
            "removed": paths,
        });
        println!("{}", serde_json::to_string_pretty(&json)?);
    } else {
        eprintln!(
            "Removed managed Codex setup from {} artifacts.",
            paths.len()
        );
    }
    Ok(())
}

#[cfg(test)]
#[path = "cmd_codex_remove.tests.rs"]
mod tests;
