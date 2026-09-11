use std::{
    fs::{File, OpenOptions, TryLockError},
    io::{self, Read, Seek, Write},
    path::{Path, PathBuf},
};

use super::WorkspaceGeneration;

/// Immutable compatibility marker. The retained exclusive-create owner sees
/// a foreign host and refuses it, even after this native owner has exited.
/// Native ownership is decided only by the OS lock, never this dummy pid.
const NATIVE_MARKER: &[u8] = b"{\"pid\":0,\"host\":\"magistr-native-os-lock/v1\"}\n";

/// One admitted workspace process, its stable OS-held lock and its fencing
/// generation. The immutable marker keeps legacy owners from deleting the
/// lock inode; native admission never reads a pid or signals another process.
#[derive(Debug)]
pub struct ServerLock {
    file: File,
    path: PathBuf,
    generation: WorkspaceGeneration,
}

impl ServerLock {
    pub const FILE_NAME: &'static str = "server.lock";

    /// Take the lock and advance its generation before any workspace store
    /// opens. Legacy, empty and unknown lock formats refuse with `AlreadyExists`:
    /// migration requires all workspace owners to be stopped first. A native
    /// lock file is never replaced or removed, including after process exit.
    pub fn acquire(path: impl Into<PathBuf>) -> io::Result<Self> {
        let path = path.into();
        ensure_marker(&path)?;
        let mut options = OpenOptions::new();
        options.read(true).write(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.custom_flags(libc::O_NOFOLLOW);
        }
        let mut file = options.open(&path)?;
        if !file.metadata()?.is_file() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("{} is not a regular lock file", path.display()),
            ));
        }
        // Only a validated native marker permits attachment on contention.
        // Unix locks are advisory, so inspect the immutable bytes first. On
        // Windows the lock denies reads through another handle; that platform
        // keeps exclusive admission and validates after taking the lock.
        #[cfg(unix)]
        validate_marker(&mut file, &path)?;
        file.try_lock().map_err(|error| match error {
            TryLockError::WouldBlock => io::Error::new(
                io::ErrorKind::WouldBlock,
                format!(
                    "another workspace server already owns this workspace (lock: {})",
                    path.display()
                ),
            ),
            TryLockError::Error(error) => error,
        })?;
        validate_marker(&mut file, &path)?;
        let generation = WorkspaceGeneration::advance(&path)?;
        Ok(Self {
            file,
            path,
            generation,
        })
    }

    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// This admission's incarnation, independent of workflow writer roles.
    #[must_use]
    pub const fn generation(&self) -> WorkspaceGeneration {
        self.generation
    }
}

/// Publish a complete marker without an empty-file window for a legacy
/// exclusive-create contender. The generation zero exists durably first, so
/// a missing counter beside an existing marker can never silently reset it.
fn ensure_marker(path: &Path) -> io::Result<()> {
    match std::fs::symlink_metadata(path) {
        Ok(metadata) if metadata.is_symlink() || !metadata.is_file() => {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("{} is not a regular lock file", path.display()),
            ));
        }
        Ok(_) => return Ok(()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => return Err(error),
    }
    WorkspaceGeneration::bootstrap(path)?;
    let parent = WorkspaceGeneration::parent(path);
    let mut staged = tempfile::NamedTempFile::new_in(parent)?;
    staged.write_all(NATIVE_MARKER)?;
    staged.as_file().sync_all()?;
    match staged.persist_noclobber(path) {
        Ok(_) => WorkspaceGeneration::sync_parent(path),
        Err(error) if error.error.kind() == io::ErrorKind::AlreadyExists => Ok(()),
        Err(error) => Err(error.error),
    }
}

/// Bound both reads and rewind before the post-admission revalidation.
fn validate_marker(file: &mut File, path: &Path) -> io::Result<()> {
    file.rewind()?;
    let mut marker = Vec::new();
    file.take(NATIVE_MARKER.len() as u64 + 1)
        .read_to_end(&mut marker)?;
    if marker != NATIVE_MARKER {
        return Err(migration_refusal(path));
    }
    Ok(())
}

fn migration_refusal(path: &Path) -> io::Error {
    io::Error::new(
        io::ErrorKind::AlreadyExists,
        format!(
            "workspace owner lock {} uses a legacy or unrecognized format; startup refused \
             before opening workspace stores. Stop every workspace owner for offline migration of the \
             obsolete lock before restarting; never remove a live owner's lock",
            path.display()
        ),
    )
}

impl Drop for ServerLock {
    fn drop(&mut self) {
        // A concurrently spawned child can inherit this descriptor until it
        // execs; closing ours alone would leave the lock held through that
        // reference. Explicit unlock ends ownership when the owner closes.
        let _ = self.file.unlock();
    }
}

#[cfg(test)]
#[path = "server_lock.tests.rs"]
mod tests;
