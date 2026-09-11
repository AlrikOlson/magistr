use crate::contracts::{ErrorKind, WorkspaceError};
use std::{fs::File, path::Path};

/// Open an admitted file without allowing a raced symlink in any Unix component.
/// Each descriptor pins the chosen directory while the next component is opened.
pub(super) fn open_source(root: &Path, admitted: &Path) -> Result<File, WorkspaceError> {
    let relative = admitted.strip_prefix(root).map_err(|_| {
        WorkspaceError::new(
            ErrorKind::PathOutsideWorkspace,
            "source escaped admitted root",
        )
    })?;
    #[cfg(unix)]
    {
        unix_open(root, relative)
    }
    #[cfg(not(unix))]
    {
        let _ = relative;
        // Other platforms retain the caller's before/after admission checks.
        File::open(admitted).map_err(io_error)
    }
}

#[cfg(unix)]
fn unix_open(root: &Path, relative: &Path) -> Result<File, WorkspaceError> {
    use std::{fs::OpenOptions, os::unix::fs::OpenOptionsExt, path::Component};
    if !root.is_absolute() {
        return Err(WorkspaceError::new(
            ErrorKind::PathOutsideWorkspace,
            "source root must be absolute",
        ));
    }
    let mut directory = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_CLOEXEC | libc::O_DIRECTORY | libc::O_NOFOLLOW)
        .open("/")
        .map_err(io_error)?;
    // Start at the filesystem root so replacing an ancestor of the workspace
    // itself with a link cannot redirect an otherwise relative open either.
    for component in root.components() {
        match component {
            Component::RootDir => {}
            Component::Normal(name) => {
                directory = open_component(&directory, name, true)?;
            }
            _ => {
                return Err(WorkspaceError::new(
                    ErrorKind::PathOutsideWorkspace,
                    "source root is not canonical",
                ));
            }
        }
    }
    let mut components = relative.components().peekable();
    while let Some(component) = components.next() {
        let Component::Normal(name) = component else {
            return Err(WorkspaceError::new(
                ErrorKind::PathOutsideWorkspace,
                "source contains a non-normal path component",
            ));
        };
        let is_directory = components.peek().is_some();
        let opened = open_component(&directory, name, is_directory)?;
        if !is_directory {
            return Ok(opened);
        }
        directory = opened;
    }
    Err(WorkspaceError::new(
        ErrorKind::FileNotFound,
        "source is the workspace directory",
    ))
}

#[cfg(unix)]
fn open_component(
    directory: &File,
    name: &std::ffi::OsStr,
    is_directory: bool,
) -> Result<File, WorkspaceError> {
    use std::{
        ffi::CString,
        os::{
            fd::{AsRawFd, FromRawFd},
            unix::ffi::OsStrExt,
        },
    };
    let name = CString::new(name.as_bytes()).map_err(|_| {
        WorkspaceError::new(
            ErrorKind::PathOutsideWorkspace,
            "source component contains NUL",
        )
    })?;
    let flags = libc::O_RDONLY
        | libc::O_CLOEXEC
        | libc::O_NOFOLLOW
        | if is_directory {
            libc::O_DIRECTORY
        } else {
            libc::O_NONBLOCK
        };
    // SAFETY: directory is a live owned descriptor, name is one NUL-terminated
    // component, and no creation flag requiring a mode argument is supplied.
    let descriptor = unsafe { libc::openat(directory.as_raw_fd(), name.as_ptr(), flags) };
    if descriptor < 0 {
        return Err(io_error(std::io::Error::last_os_error()));
    }
    // SAFETY: successful openat returned a fresh descriptor owned by this call.
    Ok(unsafe { File::from_raw_fd(descriptor) })
}

fn io_error(error: std::io::Error) -> WorkspaceError {
    WorkspaceError::new(
        ErrorKind::FileNotFound,
        format!("cannot open admitted source: {error}"),
    )
}

#[cfg(all(test, unix))]
#[path = "source_open.tests.rs"]
mod tests;
