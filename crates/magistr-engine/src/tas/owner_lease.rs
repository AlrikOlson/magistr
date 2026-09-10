use std::fs::{File, TryLockError};
use std::io;
use std::path::{Path, PathBuf};

use think_and_ship::infra::{Broadcaster, Domain, Persistence};

use super::tas_config::TasConfig;
use super::writer_role::WriterRole;

/// The advisory locks an admitted writer holds for as long as its authority
/// lives: the role's project lock, plus the feed publisher's lock when the
/// service binds the live socket.
///
/// Lock files are stable and never truncated, removed or replaced. Unlinking
/// one would let another process open a second inode and defeat exclusion;
/// the operating system releases the lock when the last handle closes or the
/// process exits, which is the whole of "orderly release".
pub(crate) struct OwnerLease {
    files: Vec<File>,
}

impl OwnerLease {
    /// Take `role`'s lock for `project_id` on the store `tas` names.
    ///
    /// # Errors
    /// `WouldBlock` when another writer of the same role holds it; any other
    /// I/O failure creating or opening the lock file. Nothing is opened after
    /// a refusal.
    pub(crate) fn acquire(tas: &TasConfig, project_id: &str, role: WriterRole) -> io::Result<Self> {
        let path = if tas.persist {
            Persistence::new(&tas.persistence_config(), Domain::Roadmap)
                .path_for(project_id)
                .with_extension(role.lock_suffix())
        } else {
            tas.project_root.join(".magistr").join(role.lock_suffix())
        };
        let mut lease = Self { files: Vec::new() };
        lease.lock(&path, role.resource())?;
        Ok(lease)
    }

    fn lock(&mut self, path: &Path, resource: &str) -> io::Result<()> {
        let failure = |error: io::Error| {
            io::Error::new(
                error.kind(),
                format!(
                    "magistr: cannot acquire {resource} at {}: {error}; \
                 startup refused before opening project engines. Close the existing writer \
                 before retrying; do not delete its lock file.",
                    path.display()
                ),
            )
        };
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(failure)?;
        }
        let file = File::options()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(path)
            .map_err(failure)?;
        file.try_lock().map_err(|error| {
            failure(match error {
                TryLockError::WouldBlock => io::Error::new(
                    io::ErrorKind::WouldBlock,
                    "another Magistr writer holds this resource",
                ),
                TryLockError::Error(error) => error,
            })
        })?;
        self.files.push(file);
        Ok(())
    }

    /// Lock the publisher for `path`, confirm nothing live is listening, and
    /// bind the broadcast socket. The publisher lock is held beside the
    /// project lock so cooperating services on different stores cannot race
    /// for one socket path.
    pub(crate) async fn broadcaster(&mut self, path: &Path) -> io::Result<Broadcaster> {
        let mut lock_path = path.as_os_str().to_os_string();
        lock_path.push(".mcp-owner.lock");
        self.lock(&PathBuf::from(lock_path), "live-feed socket owner")?;
        ensure_feed_available(path).await?;
        Broadcaster::spawn(path.to_path_buf()).ok_or_else(|| {
            io::Error::other(format!(
                "magistr: cannot bind live-feed socket {}; startup refused \
             before opening project engines (check the path and runtime support)",
                path.display()
            ))
        })
    }
}

#[cfg(unix)]
async fn ensure_feed_available(path: &Path) -> io::Result<()> {
    use tokio::time::{Duration, timeout};

    // 0.6.0 unlinks existing sockets in Broadcaster::spawn. Only let it reclaim
    // a refused/missing endpoint. Hold the stable socket lease across this probe
    // and bind so cooperating services cannot race on different project stores.
    match timeout(
        Duration::from_millis(250),
        tokio::net::UnixStream::connect(path),
    )
    .await
    {
        Ok(Err(error))
            if matches!(
                error.kind(),
                io::ErrorKind::NotFound | io::ErrorKind::ConnectionRefused
            ) =>
        {
            Ok(())
        }
        result => {
            let detail = match result {
                Ok(Ok(_)) => "another publisher is listening".to_owned(),
                Ok(Err(error)) => error.to_string(),
                Err(_) => "availability probe timed out".to_owned(),
            };
            Err(io::Error::new(
                io::ErrorKind::AddrInUse,
                format!(
                    "magistr: live-feed socket {} is unavailable: {detail}; \
                 startup refused and existing socket preserved",
                    path.display()
                ),
            ))
        }
    }
}

#[cfg(not(unix))]
async fn ensure_feed_available(path: &Path) -> io::Result<()> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        format!(
            "magistr: live-feed socket {} is unsupported on this platform",
            path.display()
        ),
    ))
}
