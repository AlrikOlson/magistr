//! `magistr preset test` — validate a preset file and dry-run gate commands.

use crate::config::preset_fragment::UserPresetFragment;
use crate::utils::format as fmt;
use color_eyre::eyre;

/// Validate a preset file: check schema, validate fields, and optionally
/// dry-run quality gate commands to verify tool availability.
pub fn cmd_preset_test(file: &std::path::Path) -> eyre::Result<()> {
    eprintln!("{} Testing preset: {}", fmt::bold("→"), file.display());

    // Load and parse
    let fragment =
        UserPresetFragment::load(file).map_err(|e| eyre::eyre!("failed to load preset: {e}"))?;

    eprintln!(
        "  {} Parsed successfully (name: {})",
        fmt::icon_pass(),
        fmt::bold(&fragment.name)
    );

    // Schema validation
    let warnings = fragment.validate();
    let errors: Vec<_> = warnings.iter().filter(|w| w.is_error).collect();
    let warns: Vec<_> = warnings.iter().filter(|w| !w.is_error).collect();

    if !errors.is_empty() {
        for w in &errors {
            eprintln!("  {} [{}]: {}", fmt::red("✗"), w.field, w.message);
        }
    }
    if !warns.is_empty() {
        for w in &warns {
            eprintln!("  {} [{}]: {}", fmt::yellow("⚠"), w.field, w.message);
        }
    }
    if warnings.is_empty() {
        eprintln!("  {} Schema validation passed", fmt::icon_pass());
    }

    // Test extends resolution
    if let Some(extends) = &fragment.extends {
        match fragment.resolve_inheritance() {
            Ok(Some(resolved)) => {
                eprintln!(
                    "  {} Inheritance resolved: extends \"{}\" → check={}, test={}, lint={}",
                    fmt::icon_pass(),
                    extends,
                    truncate_cmd(&resolved.quality_gate.check),
                    truncate_cmd(&resolved.quality_gate.test),
                    truncate_cmd(&resolved.quality_gate.lint),
                );
            }
            Ok(None) => {}
            Err(e) => {
                eprintln!("  {} Inheritance resolution failed: {e}", fmt::red("✗"));
                return Err(eyre::eyre!("preset has inheritance errors"));
            }
        }
    }

    // Dry-run gate commands: check if the command binary exists on PATH
    if let Some(qg) = &fragment.quality_gate {
        eprintln!("\n  {} Quality gate commands:", fmt::bold("→"));
        check_gate_command("check", qg.check.as_deref());
        check_gate_command("test", qg.test.as_deref());
        check_gate_command("lint", qg.lint.as_deref());
    }

    // Format command
    if let Some(fmt_cfg) = &fragment.format
        && let Some(cmd) = &fmt_cfg.command
    {
        eprintln!("\n  {} Format command:", fmt::bold("→"));
        check_gate_command("format", Some(cmd));
    }

    // Summary
    if !errors.is_empty() {
        eprintln!(
            "\n{} Preset has {} error(s) — fix before using.",
            fmt::red("✗"),
            errors.len()
        );
        return Err(eyre::eyre!("preset validation failed"));
    }

    eprintln!(
        "\n{} Preset is valid{}",
        fmt::green("✓"),
        if warns.is_empty() {
            ".".to_string()
        } else {
            format!(" ({} warning(s)).", warns.len())
        }
    );

    Ok(())
}

/// Check if a gate command's binary is available on PATH.
fn check_gate_command(gate: &str, cmd: Option<&str>) {
    let Some(cmd) = cmd else {
        eprintln!("    {} {gate}: {}", fmt::dim("—"), fmt::dim("not set"));
        return;
    };

    if cmd.trim().is_empty() {
        eprintln!(
            "    {} {gate}: {}",
            fmt::yellow("⚠"),
            fmt::dim("empty string")
        );
        return;
    }

    let binary = cmd.split_whitespace().next().unwrap_or(cmd);
    let available = which::which(binary).is_ok();

    if available {
        eprintln!(
            "    {} {gate}: {} {}",
            fmt::icon_pass(),
            truncate_cmd(cmd),
            fmt::green("(available)")
        );
    } else {
        eprintln!(
            "    {} {gate}: {} {}",
            fmt::yellow("⚠"),
            truncate_cmd(cmd),
            fmt::yellow(&format!("({binary} not found on PATH)"))
        );
    }
}

fn truncate_cmd(cmd: &str) -> String {
    if cmd.len() > 50 {
        format!("{}…", &cmd[..49])
    } else {
        cmd.to_string()
    }
}
