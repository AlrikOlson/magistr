//! Owning the process's stderr while the cockpit owns the screen.

use std::fs::File;
use std::io::{self, Read, Seek, SeekFrom};

/// How much of a very chatty session the replay keeps: the tail, from a
/// line boundary.
const REPLAY_LIMIT: usize = 64 * 1024;

/// Uniquifies the sink's name within one process.
#[cfg(unix)]
static SINKS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// File descriptor 2, redirected into an anonymous file for as long as
/// this value lives.
///
/// Anything in the process that writes to stderr — `eprintln!`, a tracing
/// subscriber's stderr writer, the default panic hook, a child that
/// inherited the descriptor — lands in the file instead of on the screen
/// the cockpit is painting. [`OwnedStderr::restore`] puts the saved
/// descriptor back and returns what was written meanwhile, so it can be
/// replayed once the alternate screen is gone; `Drop` puts the descriptor
/// back too, so an unwinding runtime never leaves stderr pointing at the
/// file.
///
/// The sink is a file, not a pipe: a pipe fills at 64 KiB and would block
/// whoever logs; the file is unlinked as soon as it is open, so nothing
/// is left behind.
///
/// The redirect is `dup2` on descriptor 2, so it exists only on Unix;
/// elsewhere [`OwnedStderr::take`] fails with `Unsupported` and the
/// terminal that wanted it reports that instead of painting.
#[derive(Debug)]
pub struct OwnedStderr {
    #[cfg(unix)]
    saved: Option<std::os::fd::OwnedFd>,
    sink: File,
}

impl OwnedStderr {
    /// Redirect stderr into a fresh anonymous file.
    ///
    /// # Errors
    ///
    /// Stderr could not be duplicated, the sink could not be created, or
    /// the redirect failed — or, off Unix, there is no redirect to make.
    #[cfg(unix)]
    pub fn take() -> io::Result<Self> {
        let saved = rustix::io::dup(rustix::stdio::stderr())?;
        let sink = anonymous_file()?;
        rustix::stdio::dup2_stderr(&sink)?;
        Ok(Self {
            saved: Some(saved),
            sink,
        })
    }

    /// There is no descriptor 2 to redirect here.
    ///
    /// # Errors
    ///
    /// Always, with [`io::ErrorKind::Unsupported`].
    #[cfg(not(unix))]
    pub fn take() -> io::Result<Self> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "stderr cannot be redirected on this platform",
        ))
    }

    /// Put the saved stderr back and return everything written to the
    /// redirected descriptor — the tail of it, past [`REPLAY_LIMIT`].
    ///
    /// # Errors
    ///
    /// The redirect could not be undone, or the sink could not be read.
    pub fn restore(mut self) -> io::Result<Vec<u8>> {
        self.put_back()?;
        let mut bytes = Vec::new();
        self.sink.seek(SeekFrom::Start(0))?;
        self.sink.read_to_end(&mut bytes)?;
        Ok(tail(bytes))
    }

    #[cfg(unix)]
    fn put_back(&mut self) -> io::Result<()> {
        if let Some(saved) = self.saved.take() {
            rustix::stdio::dup2_stderr(&saved)?;
        }
        Ok(())
    }

    #[cfg(not(unix))]
    fn put_back(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl Drop for OwnedStderr {
    fn drop(&mut self) {
        // Nothing sensible to do with a failure here: the saved
        // descriptor may be gone with the terminal.
        let _ = self.put_back();
    }
}

/// A read-write file that no path names any more.
#[cfg(unix)]
fn anonymous_file() -> io::Result<File> {
    let dir = std::env::temp_dir();
    let mut last = None;
    for _ in 0..8 {
        let n = SINKS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let path = dir.join(format!("magistr-stderr-{}-{n}", std::process::id()));
        match std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(true)
            .open(&path)
        {
            Ok(file) => {
                std::fs::remove_file(&path)?;
                return Ok(file);
            }
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => last = Some(e),
            Err(e) => return Err(e),
        }
    }
    Err(last.unwrap_or_else(|| io::Error::other("no free name for the stderr sink")))
}

/// The last [`REPLAY_LIMIT`] bytes of `bytes`, from the first line
/// boundary inside them.
fn tail(mut bytes: Vec<u8>) -> Vec<u8> {
    if bytes.len() <= REPLAY_LIMIT {
        return bytes;
    }
    let from = bytes.len() - REPLAY_LIMIT;
    let cut = bytes[from..]
        .iter()
        .position(|&b| b == b'\n')
        .map_or(from, |nl| from + nl + 1);
    bytes.drain(..cut);
    bytes
}

#[cfg(all(test, unix))]
#[path = "owned_stderr.tests.rs"]
mod tests;
