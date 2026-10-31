use std::fs::{File, TryLockError};
use std::io;
use std::path::{Path, PathBuf};

use magistr_workflow::infra::{Broadcaster, Domain, Persistence};

use super::native_config::NativeConfig;
use super::owner_generation::OwnerGeneration;
use super::writer_role::WriterRole;

/// The lock file `role` contends on for `project_id` on the store `tas`
/// names — under the persistent store, or the project's `.magistr/` when
/// persistence is off. Also what the owner endpoint's socket is derived
/// from, so the endpoint is keyed by exactly what the lease is.
pub(crate) fn lock_path(tas: &NativeConfig, project_id: &str, _role: WriterRole) -> PathBuf {
    if tas.persist {
        Persistence::new(&tas.persistence_config(), Domain::Roadmap)
            .path_for(project_id)
            .with_extension("project-owner.lock")
    } else {
        tas.workflow_root().join(".magistr/project-owner.lock")
    }
}

/// The advisory locks an admitted writer holds for as long as its authority
/// lives: the shared project's lock, its store lock, and the publisher lock
/// when the owner binds its live feed.
///
/// Lock files are stable and never truncated, removed or replaced. Unlinking
/// one would let another process open a second inode and defeat exclusion;
/// the operating system releases the lock when the last handle closes or the
/// process exits, which is the whole of "orderly release".
pub(crate) struct OwnerLease {
    files: Vec<File>,
    /// Minted in the project lock the moment it was taken.
    generation: OwnerGeneration,
}

impl OwnerLease {
    /// Take the project lock for `project_id` on the store `tas` names.
    ///
    /// # Errors
    /// `WouldBlock` when another writer holds it; any other
    /// I/O failure creating or opening the lock file. Nothing is opened after
    /// a refusal.
    pub(crate) fn acquire(
        tas: &NativeConfig,
        project_id: &str,
        role: WriterRole,
    ) -> io::Result<Self> {
        let path = lock_path(tas, project_id, role);
        let mut lease = Self {
            files: Vec::new(),
            generation: OwnerGeneration::default(),
        };
        let project_lock = tas.workflow_root().join(".magistr/workflow-owner.lock");
        lease.lock(&project_lock, role.resource())?;
        lease.lock(&path, role.resource())?;
        // The counter lives at the head of the project lock, written under
        // it: the one place every admission of this project passes through.
        let lock = lease
            .files
            .first_mut()
            .ok_or_else(|| io::Error::other("magistr: lease taken without a lock file"))?;
        lease.generation = super::owner_generation::mint(lock).map_err(|error| {
            io::Error::new(
                error.kind(),
                format!(
                    "magistr: cannot mint the owner generation in {}: {error}",
                    project_lock.display()
                ),
            )
        })?;
        Ok(lease)
    }

    /// Which incarnation of the project's owner the lease is.
    pub(crate) const fn generation(&self) -> OwnerGeneration {
        self.generation
    }

    pub(super) fn lock(&mut self, path: &Path, resource: &str) -> io::Result<()> {
        self.files.push(lock_file(path, resource)?);
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
        let publisher = self
            .files
            .pop()
            .ok_or_else(|| io::Error::other("The feed lease was not acquired."))?;
        ensure_feed_available(path).await?;
        // Every frame this owner publishes carries its generation as the
        // feed's epoch, so a subscriber that outlives it recognises the
        // next owner's frames as a change of hands.
        let broadcaster = Broadcaster::spawn(path.to_path_buf())
            .map(|broadcaster| broadcaster.with_epoch(self.generation.get()))
            .ok_or_else(|| {
                io::Error::other(format!(
                    "magistr: cannot bind live-feed socket {}; startup refused \
             before opening project engines (check the path and runtime support)",
                    path.display()
                ))
            })?;
        self.files.push(publisher);
        Ok(broadcaster)
    }
}

/// Take one advisory lock without waiting; the lock lasts as long as the
/// returned handle. A lock the lease does not keep must be dropped by its
/// taker, or this process's own transactions on that file wait forever.
pub(super) fn lock_file(path: &Path, resource: &str) -> io::Result<File> {
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
    Ok(file)
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
