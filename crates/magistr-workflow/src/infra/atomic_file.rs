//! Publish one saved file whole or not at all, on every platform the store
//! runs on.
//!
//! Everything here goes through the standard library's file operations. They
//! accept any path length on every platform, where a rename made through the
//! operating system's own interface refuses a Windows path longer than 260
//! characters; saved check results live deeper than that.

use std::{
    fs::{self, File},
    io::{self, Write},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

static NEXT_TEMPORARY: AtomicU64 = AtomicU64::new(0);

/// A file written beside its destination and renamed into place once its
/// content is durable. A crash at any instant leaves the destination as it
/// was, or whole and new, and at most a stray temporary beside it.
pub struct AtomicFile {
    path: PathBuf,
    file: File,
    published: bool,
}

impl AtomicFile {
    /// Names every unpublished temporary starts with.
    pub const TEMPORARY_PREFIX: &'static str = ".tmp-";

    /// Replace or create `path` with `bytes`.
    ///
    /// # Errors
    /// Any I/O failure; the destination is unchanged unless the rename
    /// succeeded.
    pub fn replace(path: &Path, bytes: &[u8]) -> io::Result<()> {
        Self::replace_with(path, |file| {
            file.write_all(bytes)?;
            file.sync_all()
        })
    }

    /// Create `path` with `bytes` only when nothing is there.
    ///
    /// # Errors
    /// `AlreadyExists` when the destination exists, which is left untouched;
    /// any other I/O failure.
    pub fn create(path: &Path, bytes: &[u8]) -> io::Result<()> {
        let mut staged = Self::beside(path)?;
        staged.file.write_all(bytes)?;
        staged.file.sync_all()?;
        // A hard link never replaces an existing name. Filesystems without
        // hard links take the rename instead; every caller holds the lock
        // that owns the destination, so nothing else creates it in between.
        match fs::hard_link(&staged.path, path) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => Err(error),
            Err(_) if path.try_exists()? => Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                "The saved file already exists.",
            )),
            Err(_) => staged.publish(path),
        }
    }

    /// Write `bytes` at `path` itself, without a temporary and without
    /// waiting for the disk. Only for a file nothing reads until its
    /// directory is published as a whole: a crash can leave it half-written,
    /// so the caller checks it against its expected content after an
    /// interruption and calls [`Self::confirm`] before publishing. Where
    /// files are created by the thousand, the rename and the wait cost far
    /// more than the write, and the waits overlap when they are made
    /// together afterwards.
    ///
    /// # Errors
    /// Any I/O failure.
    pub fn write_in_place(path: &Path, bytes: &[u8]) -> io::Result<()> {
        Self::private()
            .create(true)
            .truncate(true)
            .open(path)?
            .write_all(bytes)
    }

    /// Wait for the disk to confirm a file written earlier.
    ///
    /// # Errors
    /// Any I/O failure.
    pub fn confirm(path: &Path) -> io::Result<()> {
        File::options().write(true).open(path)?.sync_all()
    }

    /// Replace or create `path` with what `write` puts in the temporary.
    /// `write` makes its content durable itself, so a value it observes after
    /// synchronizing describes exactly what becomes visible.
    ///
    /// # Errors
    /// `write`'s failure or any I/O failure; the destination is unchanged
    /// unless the rename succeeded.
    pub fn replace_with<T>(
        path: &Path,
        write: impl FnOnce(&mut File) -> io::Result<T>,
    ) -> io::Result<T> {
        let mut staged = Self::beside(path)?;
        let written = write(&mut staged.file)?;
        staged.publish(path)?;
        Ok(written)
    }

    /// An exclusively created temporary in the destination's directory,
    /// private to this user where the platform has file modes.
    fn beside(path: &Path) -> io::Result<Self> {
        let parent = match path.parent() {
            Some(parent) if !parent.as_os_str().is_empty() => parent,
            _ => Path::new("."),
        };
        loop {
            let sequence = NEXT_TEMPORARY.fetch_add(1, Ordering::Relaxed);
            let temporary = parent.join(format!(
                "{}{}-{sequence}",
                Self::TEMPORARY_PREFIX,
                std::process::id()
            ));
            match Self::private().create_new(true).open(&temporary) {
                Ok(file) => {
                    return Ok(Self {
                        path: temporary,
                        file,
                        published: false,
                    });
                }
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
                Err(error) => return Err(error),
            }
        }
    }

    /// Rename a saved file or directory, waiting out another program that
    /// briefly holds it. A scanner or an indexer reading a file that was
    /// just written refuses the rename for as long as it reads, which is
    /// milliseconds; a refusal that lasts is reported as it is.
    ///
    /// # Errors
    /// The rename's own failure once the wait is over.
    pub fn rename(from: &Path, to: &Path) -> io::Result<()> {
        let mut pause = RENAME_FIRST_PAUSE;
        loop {
            match fs::rename(from, to) {
                Err(error)
                    if error.kind() == io::ErrorKind::PermissionDenied
                        && pause <= RENAME_LAST_PAUSE =>
                {
                    std::thread::sleep(pause);
                    pause *= 2;
                }
                renamed => return renamed,
            }
        }
    }

    /// Read and write access to a file that, once created, is private to
    /// this user where the platform has file modes.
    fn private() -> fs::OpenOptions {
        let mut options = File::options();
        options.read(true).write(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        options
    }

    fn publish(mut self, path: &Path) -> io::Result<()> {
        Self::rename(&self.path, path)?;
        self.published = true;
        Ok(())
    }
}

/// The pauses double from the first to the last: a little over a second in
/// all before a refusal is taken as final.
const RENAME_FIRST_PAUSE: std::time::Duration = std::time::Duration::from_millis(5);
const RENAME_LAST_PAUSE: std::time::Duration = std::time::Duration::from_millis(640);

impl Drop for AtomicFile {
    fn drop(&mut self) {
        if !self.published {
            let _ = fs::remove_file(&self.path);
        }
    }
}

#[cfg(test)]
#[path = "atomic_file.tests.rs"]
mod tests;
