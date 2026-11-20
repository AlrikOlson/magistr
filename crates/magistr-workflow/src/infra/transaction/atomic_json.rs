//! Synchronize the replacement and bind its stamp before making it visible.

use std::{fs, io::Write, path::Path};

use magistr_workflow_contracts::{StoreStamp, WriteError};
use serde::Serialize;

use crate::infra::{AtomicFile, store_stamp};

/// Write while the caller holds the canonical owner's stable advisory lock.
/// The lock file must never be removed; deleting it permits competing owners.
pub fn write_locked<T: Serialize>(path: &Path, value: &T) -> Result<StoreStamp, WriteError> {
    replace(path, value, sync_parent)
}

fn replace<T: Serialize>(
    path: &Path,
    value: &T,
    synchronize: impl FnOnce(&Path) -> std::io::Result<()>,
) -> Result<StoreStamp, WriteError> {
    let parent = path
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    // The stamp is read from the synchronized temporary, so it names exactly
    // the bytes the rename makes visible.
    let publish = || -> std::io::Result<StoreStamp> {
        fs::create_dir_all(parent)?;
        AtomicFile::replace_with(path, |file| {
            let mut buffered = std::io::BufWriter::new(&mut *file);
            serde_json::to_writer(&mut buffered, value).map_err(std::io::Error::other)?;
            buffered.flush()?;
            drop(buffered);
            file.sync_all()?;
            store_stamp::of_open(file)
        })
    };
    let stamp = publish().map_err(|error| before_commit(path, error))?;
    synchronize(parent).map_err(|error| {
        tracing::warn!(path = %path.display(), %error, "Workflow replacement is visible but durability is uncertain");
        WriteError::uncertain("Saving could not be confirmed. Read the saved work before retrying.")
    })?;
    Ok(stamp)
}

fn before_commit(path: &Path, error: std::io::Error) -> WriteError {
    tracing::warn!(path = %path.display(), %error, "Workflow change was not committed");
    WriteError::before_commit("Your changes could not be saved. Restore access and try again.")
}

fn sync_parent(parent: &Path) -> std::io::Result<()> {
    #[cfg(unix)]
    for directory in parent
        .ancestors()
        .filter(|path| !path.as_os_str().is_empty())
    {
        fs::File::open(directory)?.sync_all()?;
    }
    #[cfg(not(unix))]
    let _ = parent;
    Ok(())
}

#[cfg(test)]
#[path = "atomic_json.tests.rs"]
mod tests;
