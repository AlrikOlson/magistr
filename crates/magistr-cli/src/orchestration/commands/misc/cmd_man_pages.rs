use crate::cli;
use crate::error::{self, ResultExt};
use color_eyre::eyre;

pub fn cmd_man_pages(dir: &std::path::Path) -> eyre::Result<()> {
    let cmd = <cli::Cli as clap::CommandFactory>::command();
    let man = clap_mangen::Man::new(cmd);
    let mut buf = Vec::new();
    man.render(&mut buf)?;

    std::fs::create_dir_all(dir)
        .map_err(|e| error::MagistrError::io(dir, e))
        .suggest()?;
    let out_path = dir.join("magistr.1");
    std::fs::write(&out_path, buf)
        .map_err(|e| error::MagistrError::io(&out_path, e))
        .suggest()?;
    eprintln!("Man page written to {}", out_path.display());
    Ok(())
}
