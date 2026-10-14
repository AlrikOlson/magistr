//! Where an admitted writer's owner endpoint is found.

use std::path::PathBuf;

use sha2::{Digest, Sha256};

use super::native_config::{NativeConfig, fits_sun_path};
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
/// too deep falls back to `/tmp`.
#[must_use]
pub fn owner_socket_path(tas: &NativeConfig, project_id: &str, role: WriterRole) -> PathBuf {
    let lock = lock_path(tas, project_id, role);
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

/// The first sixteen hex digits of the SHA-256 of `text`.
fn hex_prefix(text: &str) -> String {
    let digest = Sha256::digest(text.as_bytes());
    digest
        .iter()
        .take(8)
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

#[cfg(test)]
#[path = "owner_socket.tests.rs"]
mod tests;
