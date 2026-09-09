//! Init-related commands: init, init --smart, init --dry-run, init TUI, init interactive.

use crate::scaffold;
use crate::utils::format as fmt;
use color_eyre::eyre;

/// Print dry-run preview as JSON.
fn print_dry_run_json(preview: &scaffold::ScaffoldPreview) -> eyre::Result<()> {
    let files: Vec<serde_json::Value> = preview
        .files
        .iter()
        .map(|f| {
            let action = match f.action {
                scaffold::FileAction::Create => "create",
                scaffold::FileAction::Overwrite => "overwrite",
                scaffold::FileAction::Unchanged => "unchanged",
                scaffold::FileAction::Preserved => "preserved",
            };
            let mut obj = serde_json::json!({
                "path": f.path,
                "action": action,
            });
            if let Some(ref diff) = f.diff {
                obj["diff"] = serde_json::Value::String(diff.clone());
            }
            obj
        })
        .collect();

    let envelope = serde_json::json!({
        "summary": {
            "create": preview.creates(),
            "overwrite": preview.overwrites(),
            "unchanged": preview.unchanged(),
            "preserved": preview.preserved(),
            "total": preview.files.len(),
        },
        "files": files,
    });

    println!("{}", serde_json::to_string_pretty(&envelope)?);
    Ok(())
}

/// Print dry-run preview as colored plain text.
fn print_dry_run_plain(preview: &scaffold::ScaffoldPreview) {
    eprintln!(
        "{} (dry run — no files will be written)\n",
        fmt::bold("📋 Scaffold preview")
    );

    for f in &preview.files {
        match f.action {
            scaffold::FileAction::Create => {
                eprintln!("  {}  (create)", fmt::green(&format!("+ {}", f.path)));
            }
            scaffold::FileAction::Overwrite => {
                eprintln!("  {}  (overwrite)", fmt::yellow(&format!("~ {}", f.path)));
            }
            scaffold::FileAction::Unchanged => {
                eprintln!("  {}  (unchanged)", fmt::dim(&format!("= {}", f.path)));
            }
            scaffold::FileAction::Preserved => {
                eprintln!(
                    "  {}  (preserved — not overwritten)",
                    fmt::cyan(&format!("● {}", f.path))
                );
            }
        }
    }

    // Print diffs for overwritten files
    let diffs: Vec<_> = preview.files.iter().filter(|f| f.diff.is_some()).collect();

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
        "{} {} create, {} overwrite, {} unchanged, {} preserved",
        fmt::bold("Summary:"),
        preview.creates(),
        preview.overwrites(),
        preview.unchanged(),
        preview.preserved(),
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
mod cmd_init_interactive_tui;
mod cmd_init_interview;
mod cmd_init_smart;

pub use cmd_init::cmd_init;
pub use cmd_init_check::cmd_init_check;
pub use cmd_init_dry_run::cmd_init_dry_run;
pub use cmd_init_interactive::cmd_init_interactive;
pub use cmd_init_interactive_tui::cmd_init_interactive_tui;
pub use cmd_init_interview::cmd_init_interview;
pub use cmd_init_smart::cmd_init_smart;
