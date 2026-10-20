use std::{
    fs::{File, OpenOptions},
    io::{self, Read, Seek, Write},
    path::{Path, PathBuf},
};

use super::ServerLock;
use crate::journal::JournalDb;

const ORIGINAL: &[u8] = b"{\"pid\":0,\"host\":\"magistr-native-os-lock/v1\"}\n";
const RETIRED: &[u8] = b"magistr: history moved to .magistr/workspace\n";

/// Holds the older workspace's actual OS lock during import. It never signals
/// a process, copies transient state, or removes a lock inode.
pub(super) struct WorkspaceUpgrade {
    file: File,
    journal: PathBuf,
    marker: Vec<u8>,
}

impl WorkspaceUpgrade {
    pub(super) fn acquire(root: &Path) -> io::Result<Option<Self>> {
        let directory = root.join(".workbench");
        let metadata = match std::fs::symlink_metadata(&directory) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(error),
        };
        if !metadata.is_dir() || metadata.is_symlink() {
            return Err(io::Error::other(
                "Cannot upgrade .workbench: expected a real directory; original left unchanged.",
            ));
        }
        let journal = directory.join("journal.db");
        match std::fs::symlink_metadata(&journal) {
            Ok(metadata) if !metadata.is_file() || metadata.is_symlink() => {
                return Err(io::Error::other(
                    "Cannot upgrade .workbench: its journal must be a regular file.",
                ));
            }
            Ok(_) => {}
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(error),
        }
        let path = directory.join(ServerLock::FILE_NAME);
        if !path.try_exists()? {
            drop(ServerLock::acquire(&path)?);
        }
        let mut options = OpenOptions::new();
        options.read(true).write(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.custom_flags(libc::O_NOFOLLOW);
        }
        let mut file = options.open(&path)?;
        if !file.metadata()?.is_file() {
            return Err(io::Error::other(
                "Cannot upgrade .workbench: its lock must be a regular file.",
            ));
        }
        crate::os_file_lock::try_lock_exclusive(&file).map_err(|error| {
            if error.kind() == io::ErrorKind::WouldBlock {
                io::Error::new(io::ErrorKind::WouldBlock,
                    "An older agent session is still using .workbench. Close that session and retry; your history has not been moved.")
            } else { error }
        })?;
        let mut marker = Vec::new();
        Read::by_ref(&mut file)
            .take(4096)
            .read_to_end(&mut marker)?;
        let mut retired = RETIRED.to_vec();
        retired.resize(ORIGINAL.len(), b' ');
        // A torn retirement write is recoverable only after the journal proves
        // the corresponding import committed. It cannot admit an older server.
        let known = (0..=retired.len()).any(|n| {
            marker.len() == ORIGINAL.len()
                && marker[..n] == retired[..n]
                && marker[n..] == ORIGINAL[n..]
        });
        if !known {
            return Err(io::Error::other(format!(
                "Cannot upgrade {}: unrecognized ownership record. Original history is untouched; close the older session and restore its known ownership record before retrying.",
                directory.display()
            )));
        }
        Ok(Some(Self {
            file,
            journal,
            marker,
        }))
    }

    pub(super) fn finish(mut self, journal: &JournalDb) -> Result<(), super::WorkspaceOpenError> {
        let completed = journal
            .workbench_imported()
            .map_err(super::WorkspaceOpenError::journal)?;
        if !completed && self.marker != ORIGINAL {
            return Err(io::Error::other(
                "The older history was already moved, but the current import record is missing. Restore .magistr/workspace from its backup; both folders have been left unchanged."
            ).into());
        }
        if !completed {
            journal
                .import_workbench(&self.journal)
                .map_err(super::WorkspaceOpenError::journal)?;
        }
        let mut retired = RETIRED.to_vec();
        retired.resize(ORIGINAL.len(), b' ');
        if self.marker != retired {
            self.file.rewind()?;
            self.file.write_all(&retired)?;
            self.file.sync_all()?;
        }
        Ok(())
    }
}

impl Drop for WorkspaceUpgrade {
    fn drop(&mut self) {
        let _ = crate::os_file_lock::unlock(&self.file);
    }
}

#[cfg(test)]
#[path = "workspace_upgrade.tests.rs"]
mod tests;
