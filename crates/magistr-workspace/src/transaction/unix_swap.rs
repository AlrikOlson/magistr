use crate::file_content::FileContent;
use std::{
    ffi::{CString, OsStr},
    fs::{File, OpenOptions, Permissions},
    io::{self, Write},
    os::{
        fd::{AsRawFd, FromRawFd},
        unix::{ffi::OsStrExt, fs::OpenOptionsExt},
    },
    path::{Component, Path},
};

// Pin every directory component without following symlinks, then create and
// rename the sibling temporary by descriptor. The caller has already created
// and recorded any missing parents of a written file; deletion never unlinks,
// it renames into the workspace trash so the bytes remain a physical artifact.
pub(super) fn replace(
    root: &Path,
    path: &Path,
    content: Option<&FileContent>,
    permissions: Option<&Permissions>,
) -> io::Result<()> {
    let parent = path.parent().ok_or_else(|| io::Error::other("no parent"))?;
    parent
        .strip_prefix(root)
        .map_err(|_| io::Error::other("outside root"))?;
    let directory = open_parent(parent)?;
    let target = name(
        path.file_name()
            .ok_or_else(|| io::Error::other("no filename"))?,
    )?;
    if let Some(content) = content {
        let mut random = [0_u8; 16];
        getrandom::fill(&mut random).map_err(io::Error::other)?;
        let temporary = CString::new(format!(
            ".magistr-swap-{:032x}",
            u128::from_ne_bytes(random)
        ))
        .map_err(io::Error::other)?;
        // SAFETY: live pinned directory, NUL-terminated single component, explicit
        // restrictive creation mode, and a fresh uniquely owned returned fd.
        let fd = unsafe {
            libc::openat(
                directory.as_raw_fd(),
                temporary.as_ptr(),
                libc::O_WRONLY | libc::O_CREAT | libc::O_EXCL | libc::O_NOFOLLOW | libc::O_CLOEXEC,
                0o600,
            )
        };
        if fd < 0 {
            return Err(io::Error::last_os_error());
        }
        // SAFETY: successful openat returned a fresh owned descriptor.
        let mut file = unsafe { File::from_raw_fd(fd) };
        let result = (|| {
            file.write_all(content.as_bytes())?;
            if let Some(permissions) = permissions {
                file.set_permissions(permissions.clone())?;
            }
            file.sync_all()?;
            // SAFETY: both names are single components relative to the same live
            // directory descriptor; rename never follows the replaced leaf.
            let result = unsafe {
                libc::renameat(
                    directory.as_raw_fd(),
                    temporary.as_ptr(),
                    directory.as_raw_fd(),
                    target.as_ptr(),
                )
            };
            if result < 0 {
                return Err(io::Error::last_os_error());
            }
            directory.sync_all()
        })();
        // SAFETY: owned temp filename in the pinned parent; ENOENT after rename
        // is expected, and no other path can be removed by this cleanup.
        unsafe { libc::unlinkat(directory.as_raw_fd(), temporary.as_ptr(), 0) };
        result
    } else {
        let trash_path = super::trash::directory(root)?;
        let trash = open_parent(&trash_path)?;
        let trashed = name(&super::trash::entry_name(path)?)?;
        // SAFETY: both directories are live pinned descriptors and both names
        // are single NUL-terminated components; rename never follows the leaf.
        let result = unsafe {
            libc::renameat(
                directory.as_raw_fd(),
                target.as_ptr(),
                trash.as_raw_fd(),
                trashed.as_ptr(),
            )
        };
        if result < 0 {
            return Err(io::Error::last_os_error());
        }
        directory.sync_all()?;
        trash.sync_all()
    }
}
fn open_parent(path: &Path) -> io::Result<File> {
    let mut directory = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC)
        .open("/")?;
    for component in path.components() {
        match component {
            Component::RootDir => {}
            Component::Normal(component) => {
                let component = name(component)?;
                // SAFETY: parent fd is live; component is a single C string;
                // no creation flag is supplied; successful fd is newly owned.
                let fd = unsafe {
                    libc::openat(
                        directory.as_raw_fd(),
                        component.as_ptr(),
                        libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC,
                    )
                };
                if fd < 0 {
                    return Err(io::Error::last_os_error());
                }
                // SAFETY: successful openat returned a fresh owned descriptor.
                directory = unsafe { File::from_raw_fd(fd) };
            }
            _ => return Err(io::Error::other("noncanonical parent")),
        }
    }
    Ok(directory)
}
fn name(value: &OsStr) -> io::Result<CString> {
    CString::new(value.as_bytes()).map_err(io::Error::other)
}
