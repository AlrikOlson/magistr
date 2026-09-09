use crate::config;
use crate::error::{self, ResultExt};
use crate::utils::format as fmt;
use color_eyre::eyre;

pub fn cmd_migrate(dir: &std::path::Path) -> eyre::Result<()> {
    let dir = dir
        .canonicalize()
        .map_err(|e| error::MagistrError::io(dir, e))
        .suggest()?;

    let check = config::migration::check_migration(&dir).suggest()?;

    if !check.needs_migration() {
        eprintln!("{}", fmt::success("Configuration is up to date."));
        return Ok(());
    }

    // Run auto-fixable migrations
    if !check.actions.is_empty() {
        let completed = config::migration_exec::run_migration(&dir, &check).suggest()?;
        for desc in &completed {
            eprintln!("  {} {desc}", fmt::green("✓"));
        }
        if !completed.is_empty() {
            eprintln!();
            eprintln!(
                "{}",
                fmt::success(&format!(
                    "Auto-migrated {} item{}.",
                    completed.len(),
                    if completed.len() == 1 { "" } else { "s" }
                ))
            );
        }
    }

    // Report manual-only issues
    if !check.manual_reasons.is_empty() {
        if !check.actions.is_empty() {
            eprintln!();
        }
        eprintln!("Remaining issues (require `magistr init`):");
        for reason in &check.manual_reasons {
            eprintln!("  - {reason}");
        }
        std::process::exit(1);
    }

    Ok(())
}
