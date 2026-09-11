use std::{
    fs::OpenOptions,
    io::{self, Read, Write},
    path::{Path, PathBuf},
};

/// Which admission owns the workspace. This counter is independent of the
/// workflow Host/Service roles, index progress, journal sequence and source
/// freshness. Only a held workspace OS lock may advance it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct WorkspaceGeneration(u64);

const WIDTH: usize = 20;

impl WorkspaceGeneration {
    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }

    #[cfg(test)]
    pub(super) const fn new(value: u64) -> Self {
        Self(value)
    }

    /// Prepare generation zero before the immutable lock marker exists.
    /// Atomic no-clobber publication preserves another initial contender.
    pub(super) fn bootstrap(lock: &Path) -> io::Result<()> {
        let path = Self::path(lock);
        match std::fs::symlink_metadata(&path) {
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                let mut staged = tempfile::NamedTempFile::new_in(Self::parent(&path))?;
                write!(staged, "{:0WIDTH$}", 0)?;
                staged.as_file().sync_all()?;
                match staged.persist_noclobber(&path) {
                    Ok(_) => Self::sync_parent(&path)?,
                    Err(error) if error.error.kind() == io::ErrorKind::AlreadyExists => {}
                    Err(error) => return Err(error.error),
                }
            }
            Err(error) => return Err(error),
            Ok(_) => {}
        }
        if Self::read(&path)? != 0 {
            // A first-start contender may have published the marker and
            // admitted an owner since ensure_marker observed its absence.
            // Let the caller's OS lock and marker validation decide now.
            match std::fs::symlink_metadata(lock) {
                Ok(_) => return Ok(()),
                Err(error) if error.kind() == io::ErrorKind::NotFound => {}
                Err(error) => return Err(error),
            }
            return Err(io::Error::new(
                io::ErrorKind::WouldBlock,
                format!(
                    "workspace lock {} is missing beside an existing generation; startup \
                     refused. Stop every workspace owner and repair this state offline; \
                     never replace or remove a live owner's lock",
                    lock.display()
                ),
            ));
        }
        Ok(())
    }

    /// Read strictly, advance under the held server lock, and sync a complete
    /// replacement before admitting the owner. Atomic sidecar replacement
    /// avoids a crash leaving a torn but numerically valid in-place counter.
    pub(super) fn advance(lock: &Path) -> io::Result<Self> {
        let path = Self::path(lock);
        let previous = Self::read(&path)?;
        let next = previous
            .checked_add(1)
            .ok_or_else(|| Self::invalid(&path, "the generation counter is exhausted"))?;
        let mut staged = tempfile::NamedTempFile::new_in(Self::parent(&path))?;
        write!(staged, "{next:0WIDTH$}")?;
        staged.as_file().sync_all()?;
        staged.persist(&path).map_err(|error| error.error)?;
        Self::sync_parent(&path)?;
        Ok(Self(next))
    }

    fn read(path: &Path) -> io::Result<u64> {
        let metadata = std::fs::symlink_metadata(path).map_err(|error| {
            if error.kind() == io::ErrorKind::NotFound {
                Self::invalid(path, "the generation counter is missing")
            } else {
                error
            }
        })?;
        if metadata.is_symlink() || !metadata.is_file() {
            return Err(Self::invalid(path, "the generation is not a regular file"));
        }
        let mut bytes = Vec::with_capacity(WIDTH + 1);
        let mut options = OpenOptions::new();
        options.read(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.custom_flags(libc::O_NOFOLLOW);
        }
        let file = options.open(path)?;
        if !file.metadata()?.is_file() {
            return Err(Self::invalid(path, "the generation is not a regular file"));
        }
        file.take(WIDTH as u64 + 1).read_to_end(&mut bytes)?;
        if bytes.len() != WIDTH || !bytes.iter().all(u8::is_ascii_digit) {
            return Err(Self::invalid(path, "the generation counter is malformed"));
        }
        bytes.iter().try_fold(0_u64, |value, digit| {
            value
                .checked_mul(10)
                .and_then(|value| value.checked_add(u64::from(digit - b'0')))
                .ok_or_else(|| Self::invalid(path, "the generation counter overflows u64"))
        })
    }

    fn invalid(path: &Path, reason: &str) -> io::Error {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "workspace generation {}: {reason}; startup refused before opening stores. \
                 Restore the generation state offline; it must not be reset",
                path.display()
            ),
        )
    }

    fn path(lock: &Path) -> PathBuf {
        lock.with_extension("generation")
    }

    pub(super) fn parent(path: &Path) -> &Path {
        path.parent()
            .filter(|parent| !parent.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."))
    }

    pub(super) fn sync_parent(path: &Path) -> io::Result<()> {
        #[cfg(unix)]
        std::fs::File::open(Self::parent(path))?.sync_all()?;
        #[cfg(not(unix))]
        let _ = path;
        Ok(())
    }
}

#[cfg(test)]
#[path = "owner_generation.tests.rs"]
mod tests;
