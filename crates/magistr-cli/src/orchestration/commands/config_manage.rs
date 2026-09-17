//! Configuration commands.
//!
//! `magistr config` and `magistr config check` validate project and user
//! configuration. `magistr config show` prints the effective merged settings.
//! Edit `~/.config/magistr/` or `.magistr/project.json` to change them.

use color_eyre::eyre;

use crate::cli;
use crate::config;
use crate::config::inputs::{InputOrigin, ProjectInputs};
use crate::env::MagistrEnv;
use crate::error::{self, ResultExt};
use crate::paths;
use crate::types;
use crate::utils::format as fmt;

/// Dispatch a `magistr config` subcommand.
///
/// A bare `magistr config` validates the configuration (`check`) — the
/// standalone settings TUI was pruned with the legacy Settings surface.
pub async fn cmd_config(
    action: Option<cli::ConfigAction>,
    _use_tui: bool,
    _env: &MagistrEnv,
    inputs: &ProjectInputs,
) -> eyre::Result<()> {
    match action {
        None => cmd_config_check(&std::path::PathBuf::from("."), inputs),
        Some(cli::ConfigAction::Check { dir }) => cmd_config_check(&dir, inputs),
        Some(cli::ConfigAction::Show { dir }) => cmd_config_show(&dir, inputs),
        Some(cli::ConfigAction::Migrate { dir }) => super::config::cmd_migrate(&dir),
    }
}

// ─── CLI-only commands ──────────────────────────────────────────────────────

/// Validate all configuration files and report issues.
fn cmd_config_check(dir: &std::path::Path, inputs: &ProjectInputs) -> eyre::Result<()> {
    let dir = dir
        .canonicalize()
        .map_err(|e| error::MagistrError::io(dir, e))
        .suggest()?;

    let mut errors: Vec<String> = Vec::new();
    let mut warnings: Vec<String> = Vec::new();
    let mut ok_count: usize = 0;

    // 1. Check project.json
    let project_json = paths::project_json(&dir);
    if project_json.exists() {
        match std::fs::read_to_string(&project_json) {
            Ok(content) => match serde_json::from_str::<types::ProjectConfig>(&content) {
                Ok(_) => {
                    ok_count += 1;
                    eprintln!("  {} .magistr/project.json", fmt::green("✓"));
                }
                Err(e) => {
                    errors.push(format!(".magistr/project.json: {e}"));
                    eprintln!("  {} .magistr/project.json — {}", fmt::red("✗"), e);
                }
            },
            Err(e) => {
                errors.push(format!(".magistr/project.json: cannot read — {e}"));
                eprintln!(
                    "  {} .magistr/project.json — cannot read: {}",
                    fmt::red("✗"),
                    e
                );
            }
        }
    } else {
        warnings.push(".magistr/project.json not found (run `magistr init`)".to_string());
        eprintln!(
            "  {} .magistr/project.json not found (run `magistr init`)",
            fmt::yellow("!")
        );
    }

    // 2. Check user config: a loaded file, an absent one (defaults apply),
    // an unlocatable one (no config directory) and an unreadable one are
    // four different answers.
    match inputs.user_config() {
        Ok(captured) => {
            ok_count += 1;
            match captured.origin {
                InputOrigin::Loaded(path) => {
                    eprintln!("  {} {}", fmt::green("✓"), path.display());
                }
                InputOrigin::Absent(path) => eprintln!(
                    "  {} user config (using defaults; none at {})",
                    fmt::green("✓"),
                    path.display()
                ),
                InputOrigin::Unlocated => eprintln!(
                    "  {} user config (using defaults; no config directory — set HOME or XDG_CONFIG_HOME)",
                    fmt::yellow("!")
                ),
            }
        }
        Err(e) => {
            errors.push(format!("user config: {e}"));
            eprintln!("  {} user config — {}", fmt::red("✗"), e);
        }
    }

    // 3. Check migration needs
    if let Ok(check) = config::migration::check_migration(&dir)
        && check.needs_migration()
    {
        for action in &check.actions {
            let msg = format!("migration: {}", action.description());
            warnings.push(msg.clone());
            eprintln!("  {} {}", fmt::yellow("!"), action.description());
        }
        for reason in &check.manual_reasons {
            warnings.push(format!("migration: {reason}"));
            eprintln!("  {} {}", fmt::yellow("!"), reason);
        }
        eprintln!(
            "\n  {}",
            fmt::dim("Run `magistr config migrate` to auto-upgrade.")
        );
    }

    // Summary
    eprintln!();
    if errors.is_empty() && warnings.is_empty() {
        eprintln!("{} All {ok_count} config file(s) valid", fmt::green("✓"));
        Ok(())
    } else if errors.is_empty() {
        eprintln!(
            "{} {ok_count} valid, {} warning(s)",
            fmt::yellow("!"),
            warnings.len()
        );
        Ok(())
    } else {
        eyre::bail!(
            "{} error(s), {} warning(s) in configuration",
            errors.len(),
            warnings.len()
        );
    }
}

/// Print the effective merged configuration.
fn cmd_config_show(dir: &std::path::Path, inputs: &ProjectInputs) -> eyre::Result<()> {
    let dir = dir
        .canonicalize()
        .map_err(|e| error::MagistrError::io(dir, e))
        .suggest()?;

    // User config
    let user_config = inputs
        .user_config()
        .map(|captured| captured.value)
        .unwrap_or_default();
    eprintln!("{}", fmt::bold("User Config"));
    eprintln!(
        "  theme:        {}",
        user_config.theme.default.as_deref().unwrap_or("(auto)")
    );
    if let Some(limit) = user_config.budget.limit {
        eprintln!("  budget.limit:  ${limit:.2}");
    }
    if let Some(warn) = user_config.budget.warn_threshold {
        eprintln!("  budget.warn:   ${warn:.2}");
    }
    if let Some(ref grid) = user_config.layout.grid {
        eprintln!("  layout.grid:   {grid}");
    }

    // Project config
    let project_json = paths::project_json(&dir);
    if project_json.exists() {
        match std::fs::read_to_string(&project_json) {
            Ok(content) => match serde_json::from_str::<types::ProjectConfig>(&content) {
                Ok(pc) => {
                    eprintln!();
                    eprintln!("{}", fmt::bold("Project Config"));
                    eprintln!("  project:       {}", pc.project.name);
                    eprintln!(
                        "  stack:         {} / {} / {}",
                        pc.stack.language, pc.stack.framework, pc.stack.package_manager
                    );
                    eprintln!("  check:         {}", pc.quality_gate.check);
                    eprintln!("  test:          {}", pc.quality_gate.test);
                    eprintln!("  lint:          {}", pc.quality_gate.lint);
                    eprintln!("  format:        {}", pc.format.command);
                    match &pc.spec {
                        Some(spec) => eprintln!("  spec_file:     {}", spec.file),
                        None => eprintln!("  spec_file:     (none — think-and-ship roadmap)"),
                    }
                }
                Err(e) => {
                    eprintln!();
                    eprintln!(
                        "{} .magistr/project.json — parse error: {}",
                        fmt::red("✗"),
                        e
                    );
                }
            },
            Err(e) => {
                eprintln!();
                eprintln!(
                    "{} .magistr/project.json — read error: {}",
                    fmt::red("✗"),
                    e
                );
            }
        }
    } else {
        eprintln!();
        eprintln!(
            "  {} No .magistr/project.json (run `magistr init`)",
            fmt::dim("project:")
        );
    }

    // Migration status
    if let Ok(check) = config::migration::check_migration(&dir)
        && check.needs_migration()
    {
        eprintln!();
        eprintln!("{}", fmt::bold("Migration"));
        for action in &check.actions {
            eprintln!("  {} {}", fmt::yellow("!"), action.description());
        }
        for reason in &check.manual_reasons {
            eprintln!("  {} {}", fmt::yellow("!"), reason);
        }
    }

    Ok(())
}

#[cfg(test)]
#[path = "config_manage.tests.rs"]
mod tests;
