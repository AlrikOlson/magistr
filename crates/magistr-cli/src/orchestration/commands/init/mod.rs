//! Guided setup and read-only previews of administrative changes.

use crate::scaffold;
use crate::scaffold::drift::DriftReport;
use crate::utils::format as fmt;

/// How much of a planned change a machine-readable preview carries.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PlanDetail {
    /// Every field, including an overwrite's unified diff (the CLI).
    Full,
    /// The judgement without the diff (a tool answer is a summary).
    Summary,
}

/// The administrative change plan as the JSON envelope every
/// machine-readable preview prints: the operation, the per-intent summary
/// and one [`planned_change_json`] object per path.
pub(crate) fn plan_json(plan: &scaffold::ChangePlan, detail: PlanDetail) -> serde_json::Value {
    let files: Vec<serde_json::Value> = plan
        .files
        .iter()
        .map(|f| {
            let mut entry = planned_change_json(f);
            if detail == PlanDetail::Summary {
                entry.as_object_mut().map(|obj| obj.remove("diff"));
            }
            entry
        })
        .collect();

    serde_json::json!({
        "operation": plan.operation.label(),
        "summary": {
            "create": plan.creates(),
            "overwrite": plan.overwrites(),
            "remove": plan.removes(),
            "unchanged": plan.unchanged(),
            "preserved": plan.preserved(),
            "refused": plan.refusals(),
            "total": plan.files.len(),
        },
        "files": files,
    })
}

/// The `--check` report: the drift between the configured archetypes and
/// the tree, and the changes an explicit `magistr repair` would apply,
/// as [`plan_json`]. The same object answers the CLI's `--json` and the
/// `magistr_init` tool's `check` mode.
pub(crate) fn check_report_json(
    drift: &DriftReport,
    repair: &scaffold::ChangePlan,
    detail: PlanDetail,
) -> serde_json::Value {
    let new_projects: Vec<serde_json::Value> = drift
        .new_projects
        .iter()
        .map(|p| {
            serde_json::json!({
                "path": p.path.display().to_string(),
                "language": p.language.as_str(),
                "framework": p.framework.map(|f| f.as_str()),
            })
        })
        .collect();
    let changes: Vec<serde_json::Value> = drift
        .changes
        .iter()
        .map(|c| {
            serde_json::json!({
                "archetype": c.archetype_id,
                "description": c.description,
            })
        })
        .collect();
    serde_json::json!({
        "clean": drift.is_clean(),
        "total_changes": drift.total(),
        "new_projects": new_projects,
        "removed_archetypes": drift.removed_archetypes,
        "changes": changes,
        "repair": plan_json(repair, detail),
    })
}

/// One planned change as the JSON object every machine-readable preview
/// (CLI `--json` and the `magistr_init` tool) prints: path, action,
/// intent, the ownership judgement, the expected state and any reason.
pub(crate) fn planned_change_json(f: &scaffold::PlannedChange) -> serde_json::Value {
    let mut obj = serde_json::json!({
        "path": f.path,
        "action": f.action.label(),
        "intent": f.intent().label(),
        "ownership": f.ownership.label(),
        "expected": f.expected().label(),
    });
    if let Some(reason) = f.reason() {
        obj["reason"] = serde_json::Value::String(reason);
    }
    if let Some(ref diff) = f.diff {
        obj["diff"] = serde_json::Value::String(diff.clone());
    }
    obj
}

/// Print an administrative change plan as colored plain text on stderr:
/// every path with its action, ownership and reason, the diffs of the
/// overwrites, and the summary line. `heading` names the preview and
/// `footer`, when given, says how to apply it.
pub(crate) fn print_plan_plain(plan: &scaffold::ChangePlan, heading: &str, footer: Option<&str>) {
    eprintln!("{}\n", fmt::bold(heading));

    for f in &plan.files {
        let owner = f.ownership.label();
        match &f.action {
            scaffold::FileAction::Create => {
                eprintln!("  {}  (create)", fmt::green(&format!("+ {}", f.path)));
            }
            scaffold::FileAction::Overwrite => {
                eprintln!(
                    "  {}  (overwrite, {owner})",
                    fmt::yellow(&format!("~ {}", f.path))
                );
            }
            scaffold::FileAction::Remove => {
                eprintln!(
                    "  {}  (remove, {owner})",
                    fmt::red(&format!("- {}", f.path))
                );
            }
            scaffold::FileAction::Unchanged => {
                eprintln!("  {}  (unchanged)", fmt::dim(&format!("= {}", f.path)));
            }
            scaffold::FileAction::Preserved => {
                eprintln!(
                    "  {}  (preserved — {})",
                    fmt::cyan(&format!("● {}", f.path)),
                    f.reason().unwrap_or_default()
                );
            }
            scaffold::FileAction::Refused(refusal) => {
                eprintln!(
                    "  {}  (refused — {refusal})",
                    fmt::red(&format!("! {}", f.path))
                );
            }
        }
    }

    // Print diffs for overwritten files
    let diffs: Vec<_> = plan.files.iter().filter(|f| f.diff.is_some()).collect();

    if !diffs.is_empty() {
        eprintln!("\n{}\n", fmt::bold("── Diffs ──"));
        for f in diffs {
            if let Some(ref diff) = f.diff {
                for line in diff.lines() {
                    if line.starts_with('+') && !line.starts_with("+++") {
                        eprintln!("{}", fmt::green(line));
                    } else if line.starts_with('-') && !line.starts_with("---") {
                        eprintln!("{}", fmt::red(line));
                    } else if line.starts_with("@@") {
                        eprintln!("{}", fmt::cyan(line));
                    } else {
                        eprintln!("{line}");
                    }
                }
                eprintln!();
            }
        }
    }

    eprintln!(
        "{} {} create, {} overwrite, {} remove, {} unchanged, {} preserved, {} refused",
        fmt::bold("Summary:"),
        plan.creates(),
        plan.overwrites(),
        plan.removes(),
        plan.unchanged(),
        plan.preserved(),
        plan.refusals(),
    );
    if let Some(footer) = footer {
        eprintln!("\n{footer}");
    }
}

mod cmd_init_check;
mod cmd_init_dry_run;
mod cmd_init_interview;

pub use cmd_init_check::cmd_init_check;
pub use cmd_init_dry_run::cmd_init_dry_run;
pub use cmd_init_interview::cmd_init_interview;

#[cfg(test)]
#[path = "mod.tests.rs"]
mod tests;
