//! Deterministic workspace rendezvous, keyed by the canonical directory.

use std::{
    io,
    path::{Path, PathBuf},
};

#[cfg(unix)]
use sha2::{Digest, Sha256};

#[cfg(unix)]
use super::tas_config::fits_sun_path;

/// Locate a workspace endpoint without opening its state or taking its lock.
/// The short path is in the workspace's private run directory. Deep roots
/// use a private directory under `/tmp`, independent of a client's runtime
/// environment so every client of the same user finds the same owner.
///
/// # Errors
/// The root cannot be canonicalized; local attachment is unsupported off Unix.
pub fn workspace_socket_path(root: &Path) -> io::Result<PathBuf> {
    let root = root.canonicalize()?;
    let beside = root.join(".workbench/run/owner.sock");
    #[cfg(unix)]
    {
        if fits_sun_path(&beside) {
            return Ok(beside);
        }
        // SAFETY: geteuid has no preconditions and does not access memory.
        let uid = unsafe { libc::geteuid() };
        let digest = Sha256::digest(root.as_os_str().as_encoded_bytes());
        let name: String = digest
            .iter()
            .take(16)
            .map(|byte| format!("{byte:02x}"))
            .collect();
        Ok(PathBuf::from(format!("/tmp/magistr-workspace-{uid}")).join(format!("{name}.sock")))
    }
    #[cfg(not(unix))]
    {
        let _ = beside;
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "magistr: the workspace endpoint needs Unix domain sockets",
        ))
    }
}

#[cfg(all(test, unix))]
#[path = "workspace_socket_path.tests.rs"]
mod tests;
