use std::fs::File;
use std::io::{self, Read};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};

use sha2::{Digest, Sha256};

use super::change_state::ChangeState;

const ENTRY_LIMIT: usize = 100_000;
const BYTE_LIMIT: u64 = 64 * 1024 * 1024;

/// Order-independent source fingerprint, held instead of a path snapshot.
/// A poll scans at most 100,000 entries and hashes at most 64 MiB. Crossing
/// either ceiling is an explicit failure/rescan hint, never a clean digest.
#[derive(Debug, PartialEq, Eq)]
pub(super) struct PollingDigest([u8; 32]);

impl PollingDigest {
    pub(super) fn capture(root: &Path, stop: &AtomicBool) -> io::Result<Self> {
        Self::capture_with_limits(root, stop, ENTRY_LIMIT, BYTE_LIMIT)
    }

    fn capture_with_limits(
        root: &Path,
        stop: &AtomicBool,
        entry_limit: usize,
        byte_limit: u64,
    ) -> io::Result<Self> {
        let filter_root = root.to_owned();
        let walker = ignore::WalkBuilder::new(root)
            .hidden(false)
            .follow_links(false)
            .require_git(false)
            .filter_entry(move |entry| ChangeState::is_relevant(&filter_root, entry.path()))
            .build();
        let mut digest = [0; 32];
        let mut bytes = 0_u64;
        for (count, entry) in walker.enumerate() {
            if stop.load(Ordering::Acquire) {
                return Err(io::Error::new(
                    io::ErrorKind::Interrupted,
                    "polling stopped",
                ));
            }
            if count >= entry_limit {
                return Err(io::Error::other("poll snapshot entry limit exceeded"));
            }
            let entry = entry.map_err(io::Error::other)?;
            let metadata = std::fs::symlink_metadata(entry.path())?;
            let mut hash = Sha256::new();
            let relative = entry.path().strip_prefix(root).map_err(io::Error::other)?;
            let path_bytes = relative.as_os_str().as_encoded_bytes();
            hash.update(path_bytes.len().to_le_bytes());
            hash.update(path_bytes);
            hash.update([
                u8::from(metadata.is_dir()),
                u8::from(metadata.is_file()),
                u8::from(metadata.is_symlink()),
            ]);
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                hash.update(metadata.permissions().mode().to_le_bytes());
            }
            #[cfg(not(unix))]
            hash.update([u8::from(metadata.permissions().readonly())]);
            if metadata.is_file() {
                hash.update(metadata.len().to_le_bytes());
                let modified = metadata
                    .modified()?
                    .duration_since(std::time::UNIX_EPOCH)
                    .map_err(io::Error::other)?;
                hash.update(modified.as_nanos().to_le_bytes());
                let mut file = File::open(entry.path())?;
                let mut buffer = [0_u8; 8192];
                loop {
                    if stop.load(Ordering::Acquire) {
                        return Err(io::Error::new(
                            io::ErrorKind::Interrupted,
                            "polling stopped",
                        ));
                    }
                    let length = file.read(&mut buffer)?;
                    if length == 0 {
                        break;
                    }
                    bytes = bytes.saturating_add(length as u64);
                    if bytes > byte_limit {
                        return Err(io::Error::other("poll snapshot byte limit exceeded"));
                    }
                    hash.update(&buffer[..length]);
                }
            } else if metadata.is_symlink() {
                hash.update(
                    std::fs::read_link(entry.path())?
                        .as_os_str()
                        .as_encoded_bytes(),
                );
            }
            // Commutative combination avoids retaining/sorting an unbounded
            // path list merely to neutralize filesystem enumeration order.
            for (total, entry_byte) in digest.iter_mut().zip(hash.finalize()) {
                *total ^= entry_byte;
            }
        }
        Ok(Self(digest))
    }
}

#[cfg(test)]
#[path = "polling_digest.tests.rs"]
mod tests;
