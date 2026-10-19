//! Socket admission and cleanup without touching another filesystem object.

use std::os::unix::fs::{DirBuilderExt, FileTypeExt, MetadataExt, PermissionsExt};
use std::{
    io,
    path::{Path, PathBuf},
    time::Duration,
};

use tokio::net::{UnixListener, UnixStream};

/// The exact socket inode this endpoint may unlink. Replaced paths and
/// non-socket files are never removed, including during shutdown.
pub(super) struct OwnerSocketFile {
    path: PathBuf,
    device: u64,
    inode: u64,
    uid: u32,
}

impl OwnerSocketFile {
    pub(super) async fn bind(
        path: &Path,
        private_directory: bool,
    ) -> io::Result<(Self, UnixListener)> {
        if let Some(parent) = path.parent() {
            if private_directory {
                private_parent(parent)?;
            } else {
                std::fs::create_dir_all(parent)?;
            }
        }
        ensure_free(path).await?;
        let listener = UnixListener::bind(path)?;
        let metadata = std::fs::symlink_metadata(path)?;
        let socket = Self {
            path: path.to_path_buf(),
            device: metadata.dev(),
            inode: metadata.ino(),
            uid: metadata.uid(),
        };
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))?;
        Ok((socket, listener))
    }

    pub(super) const fn uid(&self) -> u32 {
        self.uid
    }
}

impl Drop for OwnerSocketFile {
    fn drop(&mut self) {
        if let Ok(metadata) = std::fs::symlink_metadata(&self.path)
            && metadata.file_type().is_socket()
            && metadata.dev() == self.device
            && metadata.ino() == self.inode
        {
            let _ = std::fs::remove_file(&self.path);
        }
    }
}

fn private_parent(parent: &Path) -> io::Result<()> {
    if let Some(ancestor) = parent.parent() {
        std::fs::create_dir_all(ancestor)?;
        // A canonical root has no symlink ancestors. Its state directory is
        // the only new ancestor here; it must not redirect socket publication.
        if ancestor
            .file_name()
            .is_some_and(|name| name == ".magistr/workspace")
        {
            let metadata = std::fs::symlink_metadata(ancestor)?;
            if !metadata.is_dir() {
                return Err(io::Error::new(
                    io::ErrorKind::PermissionDenied,
                    format!(
                        "magistr: workspace state directory {} must not be a symlink",
                        ancestor.display()
                    ),
                ));
            }
        }
    }
    match std::fs::DirBuilder::new().mode(0o700).create(parent) {
        Ok(()) => {}
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
        Err(error) => return Err(error),
    }
    let metadata = std::fs::symlink_metadata(parent)?;
    // SAFETY: geteuid has no preconditions and does not access memory.
    let uid = unsafe { libc::geteuid() };
    if !metadata.is_dir() || metadata.uid() != uid || metadata.mode() & 0o777 != 0o700 {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            format!(
                "magistr: workspace endpoint directory {} must be a real directory owned by uid {uid} with mode 0700",
                parent.display()
            ),
        ));
    }
    Ok(())
}

async fn ensure_free(path: &Path) -> io::Result<()> {
    let metadata = match std::fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error),
    };
    if !metadata.file_type().is_socket() {
        return Err(io::Error::new(
            io::ErrorKind::AddrInUse,
            format!(
                "magistr: owner endpoint {} is occupied by a non-socket object; it was left untouched",
                path.display()
            ),
        ));
    }
    match tokio::time::timeout(Duration::from_millis(250), UnixStream::connect(path)).await {
        Ok(Err(error))
            if matches!(
                error.kind(),
                io::ErrorKind::NotFound | io::ErrorKind::ConnectionRefused
            ) =>
        {
            let current = match std::fs::symlink_metadata(path) {
                Ok(current) => current,
                Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
                Err(error) => return Err(error),
            };
            if current.dev() != metadata.dev() || current.ino() != metadata.ino() {
                return Err(io::Error::new(
                    io::ErrorKind::AddrInUse,
                    "magistr: owner endpoint changed during its availability probe",
                ));
            }
            std::fs::remove_file(path)
        }
        _ => Err(io::Error::new(
            io::ErrorKind::AddrInUse,
            format!(
                "magistr: another owner endpoint is listening or cannot be probed at {}",
                path.display()
            ),
        )),
    }
}
