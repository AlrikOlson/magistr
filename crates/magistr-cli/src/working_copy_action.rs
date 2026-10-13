use clap::Subcommand;
use std::path::PathBuf;

/// Actions for separate task working copies.
#[derive(Debug, Subcommand)]
pub enum WorkingCopyAction {
    /// Show every working copy, including local changes and unavailable paths
    List,
    /// Start a new working copy on its own branch
    Create {
        path: PathBuf,
        #[arg(long)]
        branch: String,
        #[arg(long, default_value = "HEAD")]
        from: String,
    },
    /// Reopen an archived working copy
    Open { path: PathBuf },
    /// Close a clean, idle working copy while keeping its files and history
    Archive { path: PathBuf },
}
