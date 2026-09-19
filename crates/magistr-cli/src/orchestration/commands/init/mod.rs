//! Init-related commands: init, init --smart, init --dry-run, init TUI, init interactive.

use crate::scaffold;
use crate::utils::format as fmt;
use color_eyre::eyre;

/// Print the administrative change plan as JSON.
fn print_dry_run_json(plan: &scaffold::ChangePlan) -> eyre::Result<()> {
    let files: Vec<serde_json::Value> = plan.files.iter().map(planned_change_json).collect();

    let envelope = serde_json::json!({
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
    });

    println!("{}", serde_json::to_string_pretty(&envelope)?);
    Ok(())
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

/// Print the administrative change plan as colored plain text.
fn print_dry_run_plain(plan: &scaffold::ChangePlan) {
    eprintln!(
        "{} (dry run — no files will be written)\n",
        fmt::bold("📋 Scaffold preview")
    );

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
    eprintln!(
        "\nRun {} to apply these changes.",
        fmt::bold("magistr init")
    );
}

mod cmd_init;
mod cmd_init_check;
mod cmd_init_dry_run;
mod cmd_init_interactive;
#[cfg(feature = "terminal")]
mod cmd_init_interactive_tui;
mod cmd_init_interview;
mod cmd_init_smart;

pub use cmd_init::cmd_init;
pub use cmd_init_check::cmd_init_check;
pub use cmd_init_dry_run::cmd_init_dry_run;
pub use cmd_init_interactive::cmd_init_interactive;
#[cfg(feature = "terminal")]
pub use cmd_init_interactive_tui::cmd_init_interactive_tui;
pub use cmd_init_interview::cmd_init_interview;
pub use cmd_init_smart::cmd_init_smart;
