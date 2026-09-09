//! `magistr preset create` — interactive wizard to build a custom preset fragment.

use std::io::{self, BufRead, Write};
use std::path::Path;

use crate::config::preset_fragment::{FragmentLayerType, MergeStrategy, UserPresetFragment};
use crate::types::{FormatConfigOverride, Language, QualityGateOverride, TestingConfigOverride};
use crate::utils::format as fmt;
use color_eyre::eyre;

/// Run the interactive preset creation wizard.
pub fn cmd_preset_create(dir: &Path, output: Option<&Path>) -> eyre::Result<()> {
    let stdin = io::stdin();
    let mut lines = stdin.lock().lines();

    eprintln!("{}", fmt::bold("Create a new preset fragment\n"));

    // Name
    eprint!("  Preset name: ");
    io::stderr().flush()?;
    let name = read_line(&mut lines)?;
    if name.trim().is_empty() {
        return Err(eyre::eyre!("preset name is required"));
    }

    // Description
    eprint!("  Description (optional): ");
    io::stderr().flush()?;
    let description = read_line(&mut lines)?;
    let description = if description.trim().is_empty() {
        None
    } else {
        Some(description.trim().to_string())
    };

    // Extends
    let lang_names: Vec<&str> = Language::ALL.iter().map(|l| l.as_str()).collect();
    eprintln!(
        "\n  Available base presets: {}",
        fmt::dim(&lang_names.join(", "))
    );
    eprint!("  Extend a built-in preset? (name or Enter to skip): ");
    io::stderr().flush()?;
    let extends_input = read_line(&mut lines)?;
    let extends = if extends_input.trim().is_empty() {
        None
    } else {
        let trimmed = extends_input.trim().to_string();
        if Language::from_str_loose(&trimmed).is_none() {
            eprintln!(
                "  {} Unknown preset '{}' — skipping extends",
                fmt::yellow("⚠"),
                trimmed
            );
            None
        } else {
            Some(trimmed)
        }
    };

    // Quality gates
    eprintln!(
        "\n  {} Quality gate commands (Enter to skip):",
        fmt::bold("→")
    );
    let check = prompt_optional("    check: ", &mut lines)?;
    let test = prompt_optional("    test:  ", &mut lines)?;
    let lint = prompt_optional("    lint:  ", &mut lines)?;

    let quality_gate = if check.is_some() || test.is_some() || lint.is_some() {
        Some(QualityGateOverride { check, test, lint })
    } else {
        None
    };

    // Format
    eprintln!("\n  {} Format config (Enter to skip):", fmt::bold("→"));
    let fmt_cmd = prompt_optional("    format command: ", &mut lines)?;
    let fmt_exts = prompt_optional("    file extensions (comma-separated): ", &mut lines)?;

    let format = if fmt_cmd.is_some() || fmt_exts.is_some() {
        Some(FormatConfigOverride {
            command: fmt_cmd,
            extensions: fmt_exts.map(|s| {
                s.split(',')
                    .map(|e| e.trim().to_string())
                    .filter(|e| !e.is_empty())
                    .collect()
            }),
            exclusions: None,
        })
    } else {
        None
    };

    // Testing
    eprintln!("\n  {} Testing config (Enter to skip):", fmt::bold("→"));
    let companion = prompt_optional(
        "    companion pattern (e.g. {name}.test.{ext}): ",
        &mut lines,
    )?;
    let test_dir = prompt_optional("    test directory (e.g. tests, alongside): ", &mut lines)?;

    let testing = if companion.is_some() || test_dir.is_some() {
        Some(TestingConfigOverride {
            companion_pattern: companion,
            test_dir,
        })
    } else {
        None
    };

    let fragment = UserPresetFragment {
        name: name.trim().to_string(),
        description,
        layer_type: FragmentLayerType::ProjectLocal,
        extends,
        merge_strategy: MergeStrategy::Replace,
        quality_gate,
        format,
        testing,
        extra_allowed_tools: vec![],
    };

    // Validate
    let warnings = fragment.validate();
    if !warnings.is_empty() {
        eprintln!();
        for w in &warnings {
            let icon = if w.is_error {
                fmt::red("✗")
            } else {
                fmt::yellow("⚠")
            };
            eprintln!("  {icon} [{}]: {}", w.field, w.message);
        }
    }

    // Determine output path
    let output_path = match output {
        Some(p) => p.to_path_buf(),
        None => {
            let presets_dir = dir.join(".magistr/presets");
            std::fs::create_dir_all(&presets_dir)?;
            let slug = crate::utils::slug::sanitize_prefix(fragment.name.trim());
            presets_dir.join(format!("{slug}.toml"))
        }
    };

    fragment.save(&output_path)?;

    eprintln!(
        "\n{} Created preset at {}",
        fmt::green("✓"),
        output_path.display()
    );
    eprintln!(
        "  {}",
        fmt::dim("Validate with: magistr preset test <file>")
    );

    Ok(())
}

fn read_line(lines: &mut io::Lines<io::StdinLock<'_>>) -> eyre::Result<String> {
    lines
        .next()
        .transpose()?
        .ok_or_else(|| eyre::eyre!("unexpected end of input"))
}

fn prompt_optional(
    prompt: &str,
    lines: &mut io::Lines<io::StdinLock<'_>>,
) -> eyre::Result<Option<String>> {
    eprint!("{prompt}");
    io::stderr().flush()?;
    let val = read_line(lines)?;
    Ok(if val.trim().is_empty() {
        None
    } else {
        Some(val.trim().to_string())
    })
}
