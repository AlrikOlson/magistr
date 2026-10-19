//! Configuration commands.
//!
//! `magistr config` and `magistr config check` validate project and user
//! configuration. `magistr config show` prints the effective merged settings.
//! Edit `~/.config/magistr/` or `.magistr/project.json` to change them.

use color_eyre::eyre;

use crate::cli;
use crate::config::inputs::{InputOrigin, ProjectInputs};
use crate::env::MagistrEnv;
use crate::error::{self, ResultExt};
use crate::paths;
use crate::utils::format as fmt;

/// Dispatch a `magistr config` subcommand.
///
/// A bare `magistr config` validates the current configuration (`check`).
pub async fn cmd_config(
    action: Option<cli::ConfigAction>,
    _env: &MagistrEnv,
    inputs: &ProjectInputs,
) -> eyre::Result<()> {
    match action {
        None => cmd_config_check(&std::path::PathBuf::from("."), inputs),
        Some(cli::ConfigAction::Check { dir }) => cmd_config_check(&dir, inputs),
        Some(cli::ConfigAction::Show { dir }) => cmd_config_show(&dir, inputs),
        Some(cli::ConfigAction::Set {
            key,
            value,
            project,
            agent,
            dir,
        }) => super::config_agent_set::config_agent_set(
            &key,
            &value,
            project,
            agent.as_deref(),
            &dir,
            inputs,
        ),
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
    let project_json = paths::project_json(&dir)?;
    match crate::config::read_project_settings(&project_json) {
        Ok(Some(_)) => {
            ok_count += 1;
            eprintln!("  {} .magistr/project.json", fmt::green("✓"));
        }
        Ok(None) => {
            warnings.push(".magistr/project.json not found (run `magistr init`)".to_string());
            eprintln!(
                "  {} .magistr/project.json not found (run `magistr init`)",
                fmt::yellow("!")
            );
        }
        Err(error) => {
            errors.push(error.to_string());
            eprintln!("  {} {error}", fmt::red("✗"));
        }
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

    if let Err(error) = crate::config::agent_preferences::AgentPreferences::load(&dir) {
        errors.push(error.to_string());
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
    let user_config = inputs.user_config()?.value;
    eprintln!("{}", fmt::bold("User Config"));
    eprintln!("  theme:        {}", user_config.theme.default.label());
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
    let project_json = paths::project_json(&dir)?;
    if let Some(pc) = crate::config::read_project_settings(&project_json)? {
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
            None => eprintln!("  spec_file:     (none — magistr roadmap)"),
        }
    } else {
        eprintln!();
        eprintln!(
            "  {} No .magistr/project.json (run `magistr init`)",
            fmt::dim("project:")
        );
    }

    let preferences = crate::config::agent_preferences::AgentPreferences::load(&dir)?;
    eprintln!(
        "Project agent preferences: {}",
        serde_json::to_string_pretty(&preferences)?
    );
    eprintln!(
        "Personal agent: {}",
        user_config
            .session
            .default_provider
            .as_deref()
            .unwrap_or("auto")
    );
    eprintln!(
        "Personal models: {}",
        serde_json::to_string_pretty(&user_config.models)?
    );
    eprintln!(
        "Personal effort: {}",
        serde_json::to_string_pretty(&user_config.efforts)?
    );
    eprintln!("Use next --show-config to see the selected values and their sources.");

    Ok(())
}

#[cfg(test)]
#[path = "config_manage.tests.rs"]
mod tests;
