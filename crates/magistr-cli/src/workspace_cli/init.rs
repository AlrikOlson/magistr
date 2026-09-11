//! `magistr workspace init`: seed run's `[commands]` allowlist into
//! `workbench.toml` from lockfile/manifest detection. A human runs this;
//! the served surface can never write `workbench.toml`, so seeding is the
//! one sanctioned machine-assisted amendment — and even it never touches an
//! existing table.

use std::path::Path;

use color_eyre::eyre;
use magistr_workspace::config::DetectedCommand;

use super::admitted_root;

/// Seed the allowlist of the workspace at `dir`, or say why nothing changed.
pub fn run_init(dir: &Path) -> eyre::Result<()> {
    let root = admitted_root(dir);
    let toml_path = root.join("workbench.toml");
    let detected = DetectedCommand::detect(&root);
    if detected.is_empty() {
        eprintln!("magistr workspace: no project commands detected — nothing seeded");
        return Ok(());
    }
    let existing = if toml_path.exists() {
        std::fs::read_to_string(&toml_path)?
    } else {
        String::new()
    };
    if existing
        .lines()
        .any(|line| line.trim_start().starts_with("[commands]"))
    {
        eprintln!(
            "magistr workspace: workbench.toml already declares [commands] — left untouched (amend by hand)"
        );
        return Ok(());
    }
    let block = DetectedCommand::render_table(&detected);
    let body = if existing.is_empty() {
        block
    } else {
        format!("{}\n\n{block}", existing.trim_end_matches('\n'))
    };
    std::fs::write(&toml_path, body)?;
    eprintln!(
        "magistr workspace: seeded {} command(s) into {}",
        detected.len(),
        toml_path.display()
    );
    for command in &detected {
        eprintln!(
            "  {} = {}  ({})",
            command.name,
            serde_json::Value::String(command.line.clone()),
            command.provenance
        );
    }
    Ok(())
}
