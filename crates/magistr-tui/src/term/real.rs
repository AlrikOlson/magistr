//! The process's own terminal, through rustix.

use std::io;
use std::os::fd::{AsFd, OwnedFd};
use std::sync::{Arc, Mutex, PoisonError};

use rustix::io::Errno;

use super::owned_stderr::OwnedStderr;
use super::raw_mode::RawMode;
use super::real_input::RealInput;
use super::restorer::Restorer;
use super::terminal::Terminal;
use super::winsize::winsize;
use magistr_cockpit::cockpit::Size;

/// The tty behind stdin and stdout, in raw mode while this value lives.
///
/// Frames go out through a duplicate of stdout with `write` retried on
/// partial writes and `EINTR`; the size comes from `TIOCGWINSZ`; the input
/// side is a duplicate of stdin handed out once as a [`RealInput`]. Drop
/// order restores the termios saved at [`RealTerminal::open`].
///
/// [`RealTerminal::stdio`] also takes the process's stderr for as long as
/// the terminal lives ([`OwnedStderr`]): nothing an engine, a logger or a
/// child prints can reach the screen behind the cockpit, and it all comes
/// back in the scrollback once the alternate screen is gone.
///
/// The reads poll the input descriptor itself (stdin, normally a pty
/// slave) — never `/dev/tty`, which `poll` does not support on macOS.
#[derive(Debug)]
pub struct RealTerminal {
    output: OwnedFd,
    input: Option<OwnedFd>,
    raw: RawMode,
    stderr: Arc<Mutex<Option<OwnedStderr>>>,
}

impl RealTerminal {
    /// Take over the terminal on stdin/stdout, and the process's stderr.
    ///
    /// # Errors
    ///
    /// Stdin is not a terminal, raw mode could not be entered, or stderr
    /// could not be redirected.
    pub fn stdio() -> io::Result<Self> {
        let input = rustix::io::dup(rustix::stdio::stdin())?;
        let output = rustix::io::dup(rustix::stdio::stdout())?;
        Self::open(input, output)?.own_stderr()
    }

    /// Take over the terminal behind `input` (read, raw mode) and `output`
    /// (written). Stderr is left alone; see [`RealTerminal::own_stderr`].
    ///
    /// # Errors
    ///
    /// `input` is not a terminal, or raw mode could not be entered.
    pub fn open(input: OwnedFd, output: OwnedFd) -> io::Result<Self> {
        if !rustix::termios::isatty(&input) {
            return Err(io::Error::new(
                io::ErrorKind::Unsupported,
                "the input is not a terminal",
            ));
        }
        let raw = RawMode::enter(&input)?;
        Ok(Self {
            output,
            input: Some(input),
            raw,
            stderr: Arc::new(Mutex::new(None)),
        })
    }

    /// Hold the process's stderr for the life of the terminal; what is
    /// written to it meanwhile is replayed by
    /// [`Terminal::replay_stray`] or the panic hook's restorer.
    ///
    /// # Errors
    ///
    /// Stderr could not be redirected.
    pub fn own_stderr(self) -> io::Result<Self> {
        *lock(&self.stderr) = Some(OwnedStderr::take()?);
        Ok(self)
    }

    /// Whether the tty is still in raw mode (it is until drop).
    ///
    /// # Errors
    ///
    /// The termios could not be read.
    pub fn is_raw(&self) -> io::Result<bool> {
        RawMode::is_raw(&self.output)
            .or_else(|_| self.input.as_ref().map_or(Ok(false), RawMode::is_raw))
    }

    /// The descriptor the raw-mode guard restores on drop.
    #[must_use]
    pub fn raw_mode_fd(&self) -> std::os::fd::BorrowedFd<'_> {
        self.raw.fd()
    }
}

impl Terminal for RealTerminal {
    type Input = RealInput;

    fn write(&mut self, bytes: &[u8]) -> io::Result<()> {
        write_all(&self.output, bytes)
    }

    fn size(&self) -> io::Result<Size> {
        winsize(&self.output).or_else(|_| match &self.input {
            Some(fd) => winsize(fd),
            None => Ok(Size::new(80, 24)),
        })
    }

    fn input(&mut self) -> io::Result<RealInput> {
        let fd = self.input.take().ok_or_else(|| {
            io::Error::new(io::ErrorKind::AlreadyExists, "the input was already taken")
        })?;
        Ok(RealInput::new(fd))
    }

    fn restorer(&self) -> io::Result<Restorer> {
        let output = rustix::io::dup(&self.output)?;
        let termios = self.raw.restorer()?;
        let stderr = Arc::clone(&self.stderr);
        Ok(Box::new(move |bytes| {
            // Best effort, in order: the screen back, the tty cooked, stderr
            // back and replayed. A failure here has nowhere to go.
            let _ = write_all(&output, bytes);
            let _ = termios.restore();
            let _ = replay(&stderr);
        }))
    }

    fn replay_stray(&mut self) -> io::Result<()> {
        replay(&self.stderr)
    }
}

/// Write every byte, retrying partial writes and `EINTR`.
fn write_all(fd: impl AsFd, mut bytes: &[u8]) -> io::Result<()> {
    while !bytes.is_empty() {
        match rustix::io::write(&fd, bytes) {
            Ok(0) => return Err(io::ErrorKind::WriteZero.into()),
            Ok(n) => bytes = &bytes[n..],
            Err(Errno::INTR | Errno::AGAIN) => {}
            Err(e) => return Err(e.into()),
        }
    }
    Ok(())
}

/// Hand stderr back, if it is still held, and write what it collected to
/// the restored descriptor.
fn replay(stderr: &Mutex<Option<OwnedStderr>>) -> io::Result<()> {
    let Some(owned) = lock(stderr).take() else {
        return Ok(());
    };
    let bytes = owned.restore()?;
    write_all(rustix::stdio::stderr(), &bytes)
}

fn lock(stderr: &Mutex<Option<OwnedStderr>>) -> std::sync::MutexGuard<'_, Option<OwnedStderr>> {
    stderr.lock().unwrap_or_else(PoisonError::into_inner)
}

#[cfg(test)]
#[path = "real.tests.rs"]
mod tests;
