//! Replace a file so a crash at any instant leaves either the old content or
//! the new — never a torn file.

use std::fs::{self, File};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

/// Write `bytes` to `path` through a temporary sibling: write, fsync the
/// file, rename over the target, fsync the directory. The rename is the
/// commit point; a crash before it leaves the previous file untouched and
/// at most a stray temporary beside it, which the next write ignores.
///
/// This makes one file's replacement atomic. It does not make two files
/// atomic together, and it proves nothing about what the content describes.
pub fn durable_write(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let name = path.file_name().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "a durable write needs a file name",
        )
    })?;
    let parent = match path.parent() {
        Some(parent) if !parent.as_os_str().is_empty() => parent.to_path_buf(),
        _ => PathBuf::from("."),
    };
    fs::create_dir_all(&parent)?;
    let temp = parent.join(format!(
        ".{}.tmp-{}",
        name.to_string_lossy(),
        std::process::id()
    ));
    let written = (|| {
        let mut file = File::create(&temp)?;
        file.write_all(bytes)?;
        file.sync_all()?;
        fs::rename(&temp, path)
    })();
    if written.is_err() {
        let _ = fs::remove_file(&temp);
    }
    written?;
    // The directory entry is what a reader finds after a crash; on unix it
    // needs its own sync to be durable. Filesystems that refuse to sync a
    // directory have already made the rename durable themselves.
    #[cfg(unix)]
    if let Ok(dir) = File::open(&parent) {
        let _ = dir.sync_all();
    }
    Ok(())
}

#[cfg(test)]
#[path = "durable_write.tests.rs"]
mod tests;
