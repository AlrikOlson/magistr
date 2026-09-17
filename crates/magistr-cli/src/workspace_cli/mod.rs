//! The operational workspace verbs (chunk arch-14c10b): `magistr workspace
//! doctor [bundle|config]`, `journal`, `key rotate`, `init`, `migrate` and
//! `snapshot`, the CLI host
//! over the native diagnostics beside the MCP host in `workspace_mcp`.
//!
//! Every verb here is the retained one: read-only and lock-free where the
//! reference was (doctor and journal answer beside a live server), the
//! retained renderings on stdout, diagnostics on stderr, and the retained
//! process exit codes — 0 healthy, 1 findings, 64 usage, 65 durable data
//! this binary cannot read, 66 missing root. Parsing and process exit live
//! here; `magistr-workspace` only renders.
mod doctor;
mod doctor_operations;
mod doctor_owner;
mod exit;
mod init;
mod journal;
mod journal_view;
mod key;
mod migrate;
mod snapshot;

use std::path::{Path, PathBuf};

use magistr_workspace::{config::SessionFlags, service::WorkspaceOpenError};

pub use doctor::run_doctor;
pub(crate) use exit::fail;
pub use init::run_init;
pub use journal::run_journal;
pub use journal_view::JournalView;
pub use key::run_key_rotate;
pub use migrate::run_migrate;
pub use snapshot::run_snapshot;

/// The absolute workspace root, or the retained exit 66.
pub(crate) fn admitted_root(dir: &Path) -> PathBuf {
    let root = std::path::absolute(dir).unwrap_or_else(|_| dir.to_path_buf());
    if !root.is_dir() {
        fail(
            WorkspaceOpenError::EXIT_NO_INPUT,
            &format!("workspace root does not exist: {}", root.display()),
        );
    }
    root
}

/// Parsed `--config key=value` flags, or the retained exit 64.
pub(crate) fn session_flags(pairs: &[String]) -> serde_json::Map<String, serde_json::Value> {
    let flags = SessionFlags::parse(pairs);
    if !flags.errors.is_empty() {
        fail(exit::EXIT_USAGE, &flags.errors.join("; "));
    }
    flags.flags
}
