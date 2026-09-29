//! The Windows owner endpoint: a named pipe only its owner's user can open.
//!
//! The listener and the connector change together because they carry one
//! contract between them — the same name, the same user, one instance
//! always waiting. The listener claims the name with the first instance (a
//! live owner or a squatter makes that fail, and the name is left alone),
//! keeps the next instance created before it hands a connected one off so a
//! client never finds the name missing, and grants the owner's user alone.
//! The connector refuses a pipe served by any other user before a greeting
//! is trusted. A pipe disappears with the process that served it, so there
//! is nothing stale to reclaim.

use std::{
    io,
    path::{Path, PathBuf},
    time::Duration,
};

use tokio::net::windows::named_pipe::{ClientOptions, NamedPipeClient, NamedPipeServer};
use windows_sys::Win32::Foundation::ERROR_PIPE_BUSY;

use super::user_sid::{self, OwnerOnlyDescriptor};

/// Accepts connections on one owner's pipe.
pub(super) struct PipeListener {
    name: PathBuf,
    descriptor: OwnerOnlyDescriptor,
    next: NamedPipeServer,
}

impl PipeListener {
    /// Claim `name` for `user`.
    ///
    /// # Errors
    /// `AddrInUse` when any instance of the name already exists — another
    /// owner is listening, or something else took the name first — which
    /// is left untouched; otherwise as `CreateNamedPipeW`.
    pub(super) fn bind(name: &Path, user: &str) -> io::Result<Self> {
        let descriptor = OwnerOnlyDescriptor::for_user(user)?;
        let next = descriptor.create(name, true).map_err(|error| {
            if error.kind() == io::ErrorKind::PermissionDenied {
                io::Error::new(
                    io::ErrorKind::AddrInUse,
                    format!(
                        "magistr: another owner endpoint is listening at {}, or the name was taken; it was left untouched",
                        name.display()
                    ),
                )
            } else {
                error
            }
        })?;
        Ok(Self {
            name: name.to_path_buf(),
            descriptor,
            next,
        })
    }

    /// The next connected client. The instance that replaces it is created
    /// before it is returned; if that fails the client is disconnected and
    /// the same instance waits again. Cancel safe: the wait for a client is.
    ///
    /// # Errors
    /// Waiting failed, or no replacement instance could be created.
    pub(super) async fn accept(&mut self) -> io::Result<NamedPipeServer> {
        self.next.connect().await?;
        match self.descriptor.create(&self.name, false) {
            Ok(fresh) => Ok(std::mem::replace(&mut self.next, fresh)),
            Err(error) => {
                let _ = self.next.disconnect();
                Err(error)
            }
        }
    }
}

/// Open the owner pipe `name`, waiting out the instant between one client
/// taking the waiting instance and its replacement appearing, and refuse a
/// pipe whose server runs as another user. The caller bounds the wait.
///
/// # Errors
/// `NotFound` when no owner serves the name; `PermissionDenied` when the
/// server is not this user's (or cannot be inspected); otherwise as
/// `CreateFileW`.
pub(super) async fn connect(name: &Path) -> io::Result<NamedPipeClient> {
    let client = loop {
        match ClientOptions::new().open(name) {
            Ok(client) => break client,
            Err(error) if error.raw_os_error() == ERROR_PIPE_BUSY.try_into().ok() => {
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
            Err(error) => return Err(error),
        }
    };
    let me = user_sid::current_user()?;
    match user_sid::server_user(&client) {
        Ok(server) if server == me => Ok(client),
        Ok(server) => Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            format!(
                "magistr: owner endpoint {} is served by another user ({server})",
                name.display()
            ),
        )),
        Err(error) => Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            format!(
                "magistr: the server of owner endpoint {} cannot be verified: {error}",
                name.display()
            ),
        )),
    }
}

/// The pipe name for `key` under `kind`, private to this user: two users
/// serving the same key never collide, and the name reveals neither.
///
/// # Errors
/// This process's user cannot be read.
pub(super) fn pipe_name(kind: &str, key: &[u8]) -> io::Result<PathBuf> {
    use sha2::{Digest, Sha256};
    let user = user_sid::current_user()?;
    let mut hasher = Sha256::new();
    hasher.update(user.as_bytes());
    hasher.update([0]);
    hasher.update(key);
    let digest: String = hasher
        .finalize()
        .iter()
        .take(16)
        .map(|byte| format!("{byte:02x}"))
        .collect();
    Ok(PathBuf::from(format!(r"\\.\pipe\magistr-{kind}-{digest}")))
}

#[cfg(test)]
#[path = "owner_pipe.tests.rs"]
mod tests;
