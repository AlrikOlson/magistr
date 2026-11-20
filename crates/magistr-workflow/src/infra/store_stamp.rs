//! Filesystem observations for the canonical store stamp.

use std::{fs, io, path::Path};

use magistr_workflow_contracts::StoreStamp;

/// Observe a regular file. Absence is the only successful result without a stamp.
#[cfg(test)]
pub(crate) fn of(path: &Path) -> io::Result<Option<StoreStamp>> {
    let file = match open_regular(path) {
        Ok(file) => file,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error),
    };
    of_open(&file).map(Some)
}

pub(crate) fn open_regular(path: &Path) -> io::Result<fs::File> {
    let mut options = fs::OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NONBLOCK);
    }
    let file = options.open(path)?;
    from_metadata(&file.metadata()?)?;
    Ok(file)
}

/// The stamp of an open regular file, read from its own handle.
pub(crate) fn of_open(file: &fs::File) -> io::Result<StoreStamp> {
    identified(file, &file.metadata()?)
}

/// The stamp of `file` as `meta`, read from that same handle, describes it.
/// The identity is the file's own, not its name's: a file renamed over
/// another keeps it, so a replacement is told from what it replaced even
/// inside one timestamp tick at the same length.
pub(crate) fn identified(file: &fs::File, meta: &fs::Metadata) -> io::Result<StoreStamp> {
    Ok(StoreStamp {
        inode: identity(file, meta)?,
        ..from_metadata(meta)?
    })
}

/// What a file's metadata alone says of it, with no identity: enough to tell
/// whether one open handle's file changed between two readings.
pub(crate) fn from_metadata(meta: &fs::Metadata) -> io::Result<StoreStamp> {
    if !meta.is_file() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "The saved work is not a file.",
        ));
    }
    Ok(StoreStamp {
        inode: 0,
        modified: meta.modified()?,
        length: meta.len(),
    })
}

#[cfg(unix)]
fn identity(_file: &fs::File, meta: &fs::Metadata) -> io::Result<u64> {
    use std::os::unix::fs::MetadataExt;
    Ok(meta.ino())
}

/// The file index Windows keeps for a file on its volume, which the standard
/// library does not yet give on a stable toolchain.
#[cfg(windows)]
fn identity(file: &fs::File, _meta: &fs::Metadata) -> io::Result<u64> {
    use std::os::windows::io::AsRawHandle;
    use windows_sys::Win32::Storage::FileSystem::{
        BY_HANDLE_FILE_INFORMATION, GetFileInformationByHandle,
    };
    let mut information = std::mem::MaybeUninit::<BY_HANDLE_FILE_INFORMATION>::zeroed();
    // SAFETY: the handle stays open for as long as `file` is borrowed, and
    // `information` is writable storage of the size the call fills.
    let filled =
        unsafe { GetFileInformationByHandle(file.as_raw_handle(), information.as_mut_ptr()) };
    if filled == 0 {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: the call succeeded, so every field is initialized; the storage
    // was zeroed beforehand, which is itself a valid value of this type.
    let information = unsafe { information.assume_init() };
    Ok((u64::from(information.nFileIndexHigh) << 32) | u64::from(information.nFileIndexLow))
}

#[cfg(not(any(unix, windows)))]
fn identity(_file: &fs::File, _meta: &fs::Metadata) -> io::Result<u64> {
    Ok(0)
}

pub(crate) fn unchanged(before: &fs::Metadata, after: &fs::Metadata) -> io::Result<bool> {
    if from_metadata(before)? != from_metadata(after)? {
        return Ok(false);
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        if (before.ctime(), before.ctime_nsec()) != (after.ctime(), after.ctime_nsec()) {
            return Ok(false);
        }
    }
    Ok(true)
}

#[cfg(test)]
#[path = "store_stamp.tests.rs"]
mod tests;
