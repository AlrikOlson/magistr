//! Where a family's store file stands on disk.

use std::fs;
use std::io;
use std::path::Path;
use std::time::SystemTime;

/// The identity of one family's store file as it stood when an engine last
/// read or wrote it: inode, modification time and length.
///
/// A cheap "did anyone else write this?" for an engine that answers its
/// memory: compare the stamp on disk with the one remembered and re-read
/// only when they differ. Every Magistr writer replaces the file by
/// renaming a fresh temporary over it, so a write always moves the inode —
/// the modification time and the length are carried for the filesystems
/// that recycle inodes or truncate timestamps. It is *not* a revision: it
/// orders nothing, says nothing about what changed, and an editor that
/// rewrites the file in place inside one timestamp tick at the same length
/// is invisible to it — direct edits of the store are outside the model
/// anyway. The four families' stamps stay four; no global stamp is built
/// over them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct StoreStamp {
    inode: u64,
    modified: SystemTime,
    length: u64,
}

impl StoreStamp {
    /// The stamp of the file at `path`; `None` when there is no file.
    pub fn of(path: &Path) -> io::Result<Option<Self>> {
        match fs::metadata(path) {
            Ok(meta) => Ok(Some(Self::from_metadata(&meta))),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(error),
        }
    }

    fn from_metadata(meta: &fs::Metadata) -> Self {
        #[cfg(unix)]
        let inode = {
            use std::os::unix::fs::MetadataExt;
            meta.ino()
        };
        #[cfg(not(unix))]
        let inode = 0;
        Self {
            inode,
            modified: meta.modified().unwrap_or(SystemTime::UNIX_EPOCH),
            length: meta.len(),
        }
    }
}

#[cfg(test)]
#[path = "store_stamp.tests.rs"]
mod tests;
