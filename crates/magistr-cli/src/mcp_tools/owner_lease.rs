use std::fs::{File, TryLockError};
use std::io;
use std::path::{Path, PathBuf};

use magistr_engine::tas::{Broadcaster, TasConfig};
use think_and_ship::infra::{Domain, Persistence};

/// Cooperative MCP admission, held until the last service clone is dropped.
/// Stable lock files are never removed: unlinking one would admit another inode.
pub(crate) struct OwnerLease {
    files: Vec<File>,
}

impl OwnerLease {
    pub(crate) fn acquire(tas: &TasConfig, project_id: &str) -> io::Result<Self> {
        let path = if tas.persist {
            Persistence::new(&tas.persistence_config(), Domain::Roadmap)
                .path_for(project_id)
                .with_extension("mcp-owner.lock")
        } else {
            tas.project_root.join(".magistr/mcp-owner.lock")
        };
        let mut lease = Self { files: Vec::new() };
        lease.lock(&path, "project owner")?;
        Ok(lease)
    }

    fn lock(&mut self, path: &Path, resource: &str) -> io::Result<()> {
        let failure = |error: io::Error| {
            io::Error::new(
                error.kind(),
                format!(
                    "magistr mcp-serve: cannot acquire {resource} at {}: {error}; \
                 startup refused before opening project engines. Close the existing server \
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
                    "another Magistr service holds this resource",
                ),
                TryLockError::Error(error) => error,
            })
        })?;
        self.files.push(file);
        Ok(())
    }

    pub(crate) async fn broadcaster(&mut self, path: &Path) -> io::Result<Broadcaster> {
        let mut lock_path = path.as_os_str().to_os_string();
        lock_path.push(".mcp-owner.lock");
        self.lock(&PathBuf::from(lock_path), "live-feed socket owner")?;
        ensure_feed_available(path).await?;
        Broadcaster::spawn(path.to_path_buf()).ok_or_else(|| {
            io::Error::other(format!(
                "magistr mcp-serve: cannot bind live-feed socket {}; startup refused \
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
                    "magistr mcp-serve: live-feed socket {} is unavailable: {detail}; \
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
            "magistr mcp-serve: live-feed socket {} is unsupported on this platform",
            path.display()
        ),
    ))
}
