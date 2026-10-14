use std::{fs::File, io, path::Path};

/// Retain the host's shared lifecycle lease until the source owner is closed.
pub(super) fn acquire(path: &Path) -> io::Result<File> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let file = File::options()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(path)?;
    file.try_lock_shared().map_err(|error| match error {
        std::fs::TryLockError::WouldBlock => io::Error::new(
            io::ErrorKind::WouldBlock,
            "This working copy is being closed. Try again after it is reopened.",
        ),
        std::fs::TryLockError::Error(error) => error,
    })?;
    Ok(file)
}

#[cfg(test)]
#[path = "lifecycle_lock.tests.rs"]
mod tests;
