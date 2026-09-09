use super::*;
use crate::cli;
use crate::error::{self, ResultExt};
use crate::utils::format as fmt;
use color_eyre::eyre;

pub fn cmd_completions_install(shell: clap_complete::Shell) -> eyre::Result<()> {
    use std::io::Write;

    let install_path = completions_install_path(shell);
    let path = match install_path {
        Some(p) => p,
        None => {
            eprintln!(
                "{}",
                fmt::error(&format!(
                    "Could not determine completions directory for {shell}"
                ))
            );
            eprintln!("  Use `magistr completions {shell}` to generate and install manually.");
            std::process::exit(1);
        }
    };

    // Generate completions into a buffer
    let mut cmd = <cli::Cli as clap::CommandFactory>::command();
    let mut buf = Vec::new();
    clap_complete::generate(shell, &mut cmd, "magistr", &mut buf);

    // Create parent directory if needed
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| error::MagistrError::io(parent, e))
            .suggest()?;
    }

    // Write completions file
    let mut file = std::fs::File::create(&path)
        .map_err(|e| error::MagistrError::io(&path, e))
        .suggest()?;
    file.write_all(&buf)
        .map_err(|e| error::MagistrError::io(&path, e))
        .suggest()?;

    eprintln!(
        "{}",
        fmt::success(&format!(
            "Installed {shell} completions to {}",
            path.display()
        ))
    );
    eprintln!(
        "  Restart your shell or run `source {}` to activate.",
        path.display()
    );

    Ok(())
}
