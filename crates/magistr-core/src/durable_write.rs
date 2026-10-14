//! Replace a file so a crash at any instant leaves either the old content or
//! the new — never a torn file.

use std::ffi::OsStr;
use std::fs::{self, File};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_TEMPORARY: AtomicU64 = AtomicU64::new(0);

/// Write `bytes` to `path` through a temporary sibling: write, fsync the
/// file, rename over the target, fsync the directory. The rename is the
/// commit point; a crash before it leaves the previous file untouched and
/// at most a stray temporary beside it, which the next write ignores.
/// Existing target permissions are applied before writing the temporary's content.
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
    let permissions = match fs::metadata(path) {
        Ok(metadata) => Some(metadata.permissions()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => None,
        Err(error) => return Err(error),
    };
    let temporary = TemporaryFile::create(&parent, name)?;
    if let Some(permissions) = permissions {
        temporary.file.set_permissions(permissions)?;
    }
    temporary.replace(path, bytes)?;
    // The replacement is visible, but its directory entry also needs a sync.
    // A failure here must not be reported as a confirmed durable save.
    #[cfg(unix)]
    File::open(&parent)?.sync_all()?;
    Ok(())
}

/// Each replacement owns an exclusively created sibling until its rename.
struct TemporaryFile {
    path: PathBuf,
    file: File,
    committed: bool,
}

impl TemporaryFile {
    fn create(parent: &Path, name: &OsStr) -> io::Result<Self> {
        loop {
            let sequence = NEXT_TEMPORARY.fetch_add(1, Ordering::Relaxed);
            let mut temporary_name = std::ffi::OsString::from(".");
            temporary_name.push(name);
            temporary_name.push(format!(".tmp-{}-{sequence}", std::process::id()));
            let path = parent.join(temporary_name);
            match File::options().write(true).create_new(true).open(&path) {
                Ok(file) => {
                    return Ok(Self {
                        path,
                        file,
                        committed: false,
                    });
                }
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
                Err(error) => return Err(error),
            }
        }
    }

    fn replace(mut self, target: &Path, bytes: &[u8]) -> io::Result<()> {
        self.file.write_all(bytes)?;
        self.file.sync_all()?;
        fs::rename(&self.path, target)?;
        self.committed = true;
        Ok(())
    }
}

impl Drop for TemporaryFile {
    fn drop(&mut self) {
        if !self.committed {
            let _ = fs::remove_file(&self.path);
        }
    }
}

#[cfg(test)]
#[path = "durable_write.tests.rs"]
mod tests;
