use std::{
    fs::{File, OpenOptions},
    io::{self, Read, Seek, Write},
    path::{Path, PathBuf},
};

use super::{LockRecord, WorkspaceGeneration};

/// Current immutable marker. Ownership is decided only by the OS lock;
/// none of the marker's bytes are used to discover or signal a process.
const NATIVE_MARKER: &[u8] = b"{\"pid\":0,\"host\":\"magistr-native-os-lock/v1\"}\n";

/// One admitted workspace process, its stable OS-held lock and its fencing
/// generation. The lock inode stays stable across admissions; admission
/// never reads a pid or signals another process.
#[derive(Debug)]
pub struct ServerLock {
    file: File,
    path: PathBuf,
    generation: WorkspaceGeneration,
}

impl ServerLock {
    pub const FILE_NAME: &'static str = "server.lock";

    /// Take the lock and advance its generation before any workspace store
    /// opens. Empty and unknown lock formats refuse with `AlreadyExists`.
    /// A lock file is never replaced or removed, including after process exit.
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
        // Inspect the immutable bytes first, then again under the lock.
        validate_marker(&mut file, &path)?;
        crate::os_file_lock::try_lock_exclusive(&file).map_err(|error| match error.kind() {
            io::ErrorKind::WouldBlock => io::Error::new(
                io::ErrorKind::WouldBlock,
                format!(
                    "another workspace server already owns this workspace (lock: {})",
                    path.display()
                ),
            ),
            _ => error,
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

    /// Read the record at `path` without locking, modifying or judging it:
    /// what a diagnostic reports beside a live owner. A native marker says
    /// nothing about whether an owner holds the lock now — only the owner's
    /// own endpoint can. Unknown bytes are reported without interpretation.
    ///
    /// # Errors
    /// `InvalidInput` for a path that is not a regular file, and I/O errors.
    pub fn inspect(path: &Path) -> io::Result<LockRecord> {
        let metadata = match std::fs::symlink_metadata(path) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                return Ok(LockRecord::Absent);
            }
            Err(error) => return Err(error),
        };
        if metadata.is_symlink() || !metadata.is_file() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("{} is not a regular lock file", path.display()),
            ));
        }
        let mut record = Vec::new();
        File::open(path)?.take(4096).read_to_end(&mut record)?;
        if record == NATIVE_MARKER {
            return Ok(LockRecord::Native);
        }
        Ok(LockRecord::Unrecognized)
    }
}

/// Publish a complete marker without an empty-file window for another
/// contender. The generation zero exists durably first, so
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
        return Err(invalid_marker_refusal(path));
    }
    Ok(())
}

fn invalid_marker_refusal(path: &Path) -> io::Error {
    io::Error::new(
        io::ErrorKind::AlreadyExists,
        format!(
            "workspace owner lock {} has an unrecognized format; admission refused \
             before opening workspace stores. Restore the workspace state from a known-good \
             backup while all workspace owners are stopped. Never remove a live owner's lock",
            path.display()
        ),
    )
}

impl Drop for ServerLock {
    fn drop(&mut self) {
        // A concurrently spawned child can inherit this descriptor until it
        // execs; closing ours alone would leave the lock held through that
        // reference. Explicit unlock ends ownership when the owner closes.
        let _ = crate::os_file_lock::unlock(&self.file);
    }
}

#[cfg(test)]
#[path = "server_lock.tests.rs"]
mod tests;
