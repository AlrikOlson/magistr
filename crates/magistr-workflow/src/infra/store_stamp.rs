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
    from_metadata(&file.metadata()?).map(Some)
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

pub(crate) fn from_metadata(meta: &fs::Metadata) -> io::Result<StoreStamp> {
    if !meta.is_file() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "The saved work is not a file.",
        ));
    }
    #[cfg(unix)]
    let inode = {
        use std::os::unix::fs::MetadataExt;
        meta.ino()
    };
    #[cfg(not(unix))]
    let inode = 0;
    Ok(StoreStamp {
        inode,
        modified: meta.modified()?,
        length: meta.len(),
    })
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
