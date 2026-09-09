use crate::cli;
use color_eyre::eyre;

pub fn cmd_completions(shell: clap_complete::Shell) -> eyre::Result<()> {
    let mut cmd = <cli::Cli as clap::CommandFactory>::command();
    clap_complete::generate(shell, &mut cmd, "magistr", &mut std::io::stdout());
    Ok(())
}
