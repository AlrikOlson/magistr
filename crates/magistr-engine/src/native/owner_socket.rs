//! Where an admitted writer's owner endpoint is found.

use std::path::PathBuf;

use sha2::{Digest, Sha256};

use super::native_config::NativeConfig;
#[cfg(not(windows))]
use super::native_config::fits_sun_path;
use super::owner_lease::lock_path;
use super::writer_role::WriterRole;

/// The Unix socket the owner of `role` on `project_id` and the store `tas`
/// names serves its endpoint on — and the one an attaching client
/// connects to. Derived, never announced: every process computes it from
/// the same three facts the lease is keyed by, so a copied project
/// identity on one store finds the same owner and a different store finds
/// none.
///
/// The socket sits beside the project's lock file (`<lock>.sock`) whenever
/// that fits the kernel's `sun_path`; a store rooted deep enough to
/// overflow it gets `<runtime_dir>/magistr-<digest>.workflow.sock`, the
/// digest being the lock path's, and a runtime directory that is itself
/// too deep falls back to `/tmp`. On Windows it is a named pipe keyed by
/// this user and the canonical lock path.
#[must_use]
pub fn owner_socket_path(tas: &NativeConfig, project_id: &str, role: WriterRole) -> PathBuf {
    let lock = canonical_location(lock_path(tas, project_id, role));
    #[cfg(windows)]
    {
        workflow_pipe_name(&lock)
    }
    #[cfg(not(windows))]
    {
        beside_or_short(tas, &lock)
    }
}

/// The pipe for the lock at `lock`. A user that cannot be read keys the
/// name by the lock alone: such a process can neither serve nor verify a
/// pipe, so the name only has to be stable, and the pipe's own access
/// control still admits its owner's user alone.
#[cfg(windows)]
fn workflow_pipe_name(lock: &std::path::Path) -> PathBuf {
    let key = lock.as_os_str().as_encoded_bytes();
    super::owner_pipe::pipe_name("workflow", key).unwrap_or_else(|_| {
        let digest = hex_prefix(&lock.to_string_lossy());
        PathBuf::from(format!(r"\\.\pipe\magistr-workflow-{digest}"))
    })
}

#[cfg(not(windows))]
fn beside_or_short(tas: &NativeConfig, lock: &std::path::Path) -> PathBuf {
    let beside = PathBuf::from(format!("{}.sock", lock.display()));
    if fits_sun_path(&beside) {
        return beside;
    }
    let digest = hex_prefix(&lock.to_string_lossy());
    let name = format!("magistr-{digest}.workflow.sock");
    let short = tas.runtime_dir.join(&name);
    if fits_sun_path(&short) {
        return short;
    }
    PathBuf::from("/tmp").join(name)
}

/// Resolve existing ancestors before hashing a rendezvous. A symlink or "."
/// spelling must find the same owner even when the store itself is not yet created.
fn canonical_location(path: PathBuf) -> PathBuf {
    let mut ancestor = path.as_path();
    let mut suffix = Vec::new();
    loop {
        if let Ok(mut canonical) = ancestor.canonicalize() {
            for component in suffix.iter().rev() {
                canonical.push(component);
            }
            return canonical;
        }
        match (ancestor.parent(), ancestor.file_name()) {
            (Some(parent), Some(name)) => {
                suffix.push(name);
                ancestor = parent;
            }
            _ => return path,
        }
    }
}

/// The first sixteen hex digits of the SHA-256 of `text`.
fn hex_prefix(text: &str) -> String {
    let digest = Sha256::digest(text.as_bytes());
    digest
        .iter()
        .take(8)
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

#[cfg(all(test, unix))]
#[path = "owner_socket.tests.rs"]
mod tests;
