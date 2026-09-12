use std::{
    fs::{File, OpenOptions},
    io::{self, Read, Seek, Write},
    path::{Path, PathBuf},
};

use super::{LegacyMigration, WorkspaceGeneration};

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

    /// The documented offline migration, as one refusing command: move an
    /// obsolete legacy or empty lock record aside (never delete it) so the
    /// native owner can admit. A native marker is never touched. The legacy
    /// record names the process that wrote it; when that host is this host
    /// and that pid is still alive the migration refuses, because the legacy
    /// server never took an OS lock this code could observe. A record from
    /// another host is refused as well: liveness cannot be judged here.
    ///
    /// # Errors
    /// `WouldBlock` for a possibly live legacy owner or a foreign host,
    /// `InvalidInput` for a path that is not a regular file, and I/O errors.
    pub fn migrate_legacy(path: &Path) -> io::Result<LegacyMigration> {
        let metadata = match std::fs::symlink_metadata(path) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                return Ok(LegacyMigration::Absent);
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
            return Ok(LegacyMigration::AlreadyNative);
        }
        if let Ok(legacy) = serde_json::from_slice::<serde_json::Value>(&record) {
            let host = legacy["host"].as_str().unwrap_or_default();
            let pid = legacy["pid"].as_u64().unwrap_or(0);
            let local = local_hostname();
            if !host.is_empty() && host != local {
                return Err(io::Error::new(
                    io::ErrorKind::WouldBlock,
                    format!(
                        "legacy workspace lock {} was written on host {host}, not {local}; migrate it there",
                        path.display()
                    ),
                ));
            }
            if pid != 0 && is_alive(pid) {
                return Err(io::Error::new(
                    io::ErrorKind::WouldBlock,
                    format!(
                        "legacy workspace server pid {pid} is still alive on this host; stop it before migrating {}",
                        path.display()
                    ),
                ));
            }
        }
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|elapsed| elapsed.as_secs())
            .unwrap_or_default();
        let backup = path.with_file_name(format!("{}.legacy-{stamp}", Self::FILE_NAME));
        if backup.exists() {
            return Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                format!("migration backup {} already exists", backup.display()),
            ));
        }
        std::fs::rename(path, &backup)?;
        WorkspaceGeneration::sync_parent(path)?;
        Ok(LegacyMigration::Migrated { backup })
    }
}

/// Whether `pid` names a live process of this host. `EPERM` means it exists
/// under another user; only `ESRCH` means it is gone.
fn is_alive(pid: u64) -> bool {
    #[cfg(unix)]
    {
        let Ok(pid) = libc::pid_t::try_from(pid) else {
            return false;
        };
        // SAFETY: signal 0 delivers nothing; it only asks whether pid exists.
        if unsafe { libc::kill(pid, 0) } == 0 {
            return true;
        }
        io::Error::last_os_error().raw_os_error() == Some(libc::EPERM)
    }
    #[cfg(not(unix))]
    {
        let _ = pid;
        true
    }
}

/// This host's name, as the retained legacy server recorded it.
fn local_hostname() -> String {
    #[cfg(unix)]
    {
        let mut buffer = [0_u8; 256];
        // SAFETY: the buffer is valid for its whole length and gethostname
        // writes at most that many bytes.
        let code =
            unsafe { libc::gethostname(buffer.as_mut_ptr().cast::<libc::c_char>(), buffer.len()) };
        if code != 0 {
            return String::new();
        }
        let end = buffer
            .iter()
            .position(|byte| *byte == 0)
            .unwrap_or(buffer.len());
        String::from_utf8_lossy(&buffer[..end]).into_owned()
    }
    #[cfg(not(unix))]
    {
        String::new()
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
        let _ = crate::os_file_lock::unlock(&self.file);
    }
}

#[cfg(test)]
#[path = "server_lock.tests.rs"]
mod tests;
