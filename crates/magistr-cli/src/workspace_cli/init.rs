//! `magistr workspace init`: seed run's `[commands]` allowlist into
//! `.magistr/workspace.toml` from lockfile/manifest detection. A human runs this;
//! the served surface can never write `.magistr/workspace.toml`, so seeding is the
//! one sanctioned machine-assisted amendment — and even it never touches an
//! existing table. The scaffold seeds through the same function.

use std::path::Path;

use color_eyre::eyre;
use magistr_workspace::config::DetectedCommand;

use super::admitted_root;

/// Seed the allowlist of the workspace at `dir`, or say why nothing changed.
pub fn run_init(dir: &Path) -> eyre::Result<()> {
    let root = admitted_root(dir);
    let toml_path = root.join(".magistr/workspace.toml");
    match seed_allowlist(&root)? {
        Seeded::Nothing => {
            eprintln!("magistr workspace: no project commands detected — nothing seeded");
        }
        Seeded::Declared => eprintln!(
            "magistr workspace: .magistr/workspace.toml already declares [commands] — left untouched (amend by hand)"
        ),
        Seeded::Commands(detected) => {
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
        }
    }
    Ok(())
}

/// What seeding found.
pub(crate) enum Seeded {
    /// No manifest or lockfile named a command.
    Nothing,
    /// The file already declares `[commands]`; nothing was written.
    Declared,
    /// These rows were written as the new table.
    Commands(Vec<DetectedCommand>),
}

/// Write the detected `[commands]` table into `root/.magistr/workspace.toml` unless
/// the file already declares one.
pub(crate) fn seed_allowlist(root: &Path) -> std::io::Result<Seeded> {
    let toml_path = root.join(".magistr/workspace.toml");
    let detected = DetectedCommand::detect(root);
    if detected.is_empty() {
        return Ok(Seeded::Nothing);
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
        return Ok(Seeded::Declared);
    }
    let block = DetectedCommand::render_table(&detected);
    let body = if existing.is_empty() {
        block
    } else {
        format!("{}\n\n{block}", existing.trim_end_matches('\n'))
    };
    std::fs::create_dir_all(root.join(".magistr"))?;
    std::fs::write(&toml_path, body)?;
    Ok(Seeded::Commands(detected))
}
