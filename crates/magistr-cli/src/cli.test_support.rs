//! Parse the real CLI schema with explicit arguments as the only input.
//!
//! Shared by library and binary unit tests; subprocess tests exercise the
//! production environment bindings without mutating the test process.

use clap::{CommandFactory, FromArgMatches};

pub(crate) fn parse_cli<I, T>(args: I) -> Result<crate::cli::Cli, clap::Error>
where
    I: IntoIterator<Item = T>,
    T: Into<std::ffi::OsString> + Clone,
{
    fn without_env(command: clap::Command) -> clap::Command {
        command
            .mut_args(|arg| arg.env(None::<&str>))
            .mut_subcommands(without_env)
    }
    let matches = without_env(crate::cli::Cli::command()).try_get_matches_from(args)?;
    crate::cli::Cli::from_arg_matches(&matches)
}
