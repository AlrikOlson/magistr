//! Unix domain socket path conventions for magistr.
//!
//! All magistr sockets live under `~/.magistr/sockets/` with `0o700` permissions
//! on the directory to prevent other users from connecting.

use std::path::{Path, PathBuf};

use super::error::TransportError;

/// Returns the default socket directory: `~/.magistr/sockets/`.
///
/// # Errors
///
/// Returns [`TransportError::Io`] if the home directory cannot be determined.
pub fn default_socket_dir() -> Result<PathBuf, TransportError> {
    let home = home_dir()?;
    Ok(socket_dir_under(&home))
}

/// Returns the conventional socket path for a named server:
/// `~/.magistr/sockets/{name}.sock`.
///
/// # Errors
///
/// Returns [`TransportError::Io`] if the home directory cannot be determined.
pub fn server_socket_path(name: &str) -> Result<PathBuf, TransportError> {
    Ok(server_socket_path_under(&home_dir()?, name))
}

/// Ensure the socket directory exists with restrictive permissions (`0o700`).
///
/// Creates `~/.magistr/sockets/` if it does not exist and sets permissions so
/// only the current user can access it.
///
/// # Errors
///
/// Returns [`TransportError::Io`] if directory creation or permission
/// setting fails.
pub fn ensure_socket_dir() -> Result<PathBuf, TransportError> {
    let home = home_dir()?;
    ensure_socket_dir_under(&home)
}

/// Compute the socket directory relative to a base (home) path.
fn socket_dir_under(base: &Path) -> PathBuf {
    base.join(".magistr").join("sockets")
}

fn server_socket_path_under(base: &Path, name: &str) -> PathBuf {
    socket_dir_under(base).join(format!("{name}.sock"))
}

/// Ensure the socket directory exists under a given base path.
fn ensure_socket_dir_under(base: &Path) -> Result<PathBuf, TransportError> {
    let dir = socket_dir_under(base);
    if !dir.exists() {
        std::fs::create_dir_all(&dir)?;
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let perms = std::fs::Permissions::from_mode(0o700);
        std::fs::set_permissions(&dir, perms)?;
    }

    Ok(dir)
}

/// Get the user's home directory.
fn home_dir() -> Result<PathBuf, TransportError> {
    home_dir_from(std::env::var("HOME"))
}

fn home_dir_from(home: Result<String, std::env::VarError>) -> Result<PathBuf, TransportError> {
    home.map(PathBuf::from).map_err(|_| {
        TransportError::Io(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "HOME environment variable not set",
        ))
    })
}

#[cfg(test)]
#[path = "socket_path.tests.rs"]
mod tests;
