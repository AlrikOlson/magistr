use crate::error::{self, ResultExt};
use crate::scaffold;
use crate::utils::format as fmt;
use color_eyre::Section;
use color_eyre::eyre;

pub fn cmd_reset(dir: &std::path::Path) -> eyre::Result<()> {
    let dir = dir
        .canonicalize()
        .map_err(|e| error::MagistrError::io(dir, e))
        .suggest()?;

    if !dir.join(".claude").exists() {
        return Err(
            eyre::eyre!("No .claude/ directory found in {}", dir.display())
                .suggestion("Run `magistr init` to scaffold a new configuration."),
        );
    }

    eprintln!(
        "{}",
        fmt::yellow("This will remove unchanged, recorded agent scaffold files.")
    );
    eprintln!("Customized and unrecorded files, CLAUDE.md, and ROADMAP.md will be preserved.");
    eprintln!(
        "The .magistr/ and legacy .jig/ directories, including local state, will be deleted."
    );
    eprintln!("Managed MCP and gitignore entries will be removed.");
    eprintln!();
    eprint!("Continue? [y/N] ");

    let mut answer = String::new();
    std::io::stdin().read_line(&mut answer)?;
    if answer.trim().to_lowercase() != "y" {
        eprintln!("{}", fmt::info("Aborted."));
        return Ok(());
    }

    scaffold::remove_scaffold(&dir).suggest()?;
    eprintln!("{}", fmt::success("Removed scaffolding"));
    eprintln!();
    eprintln!("Run {} to re-scaffold.", fmt::bold("magistr init"));

    Ok(())
}
