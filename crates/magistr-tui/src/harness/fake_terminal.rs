//! A terminal that captures output and plays a script as input.

use std::io;
use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Duration, Instant};

use super::capture::Capture;
use super::deliveries::Deliveries;
use super::fake_input::FakeInput;
use super::script::Script;
use crate::term::{OwnedStderr, Restorer, Terminal};
use magistr_cockpit::cockpit::Size;

/// The [`Terminal`] for tests: writes land in a [`Capture`], the size is
/// whatever the script last set, the input side plays the script and
/// logs when it delivered each byte step in [`Deliveries`].
///
/// [`FakeTerminal::owning_stderr`] makes it hold the process's stderr the
/// way the real terminal does; the replay then lands in the capture, after
/// the leave sequence, where a test can split the stream and check that
/// nothing foreign came before it.
#[derive(Debug)]
pub struct FakeTerminal {
    capture: Capture,
    deliveries: Deliveries,
    size: Arc<Mutex<Size>>,
    script: Option<Script>,
    stderr: Arc<Mutex<Option<OwnedStderr>>>,
    write_delay: Duration,
}

impl FakeTerminal {
    /// A terminal of `size` that will play `script`.
    #[must_use]
    pub fn new(size: Size, script: Script) -> Self {
        Self {
            capture: Capture::new(),
            deliveries: Deliveries::new(),
            size: Arc::new(Mutex::new(size)),
            script: Some(script),
            stderr: Arc::new(Mutex::new(None)),
            write_delay: Duration::ZERO,
        }
    }

    /// Every write takes `delay` — a terminal, or a machine, that is
    /// slower than the cockpit's tick, so the loop comes back from a
    /// paint to a tick already due. The write is dated from its start,
    /// so anything that arrived while it was in flight is after it.
    #[must_use]
    pub const fn slow_writes(mut self, delay: Duration) -> Self {
        self.write_delay = delay;
        self
    }

    /// Hold the process's stderr until it is replayed into the capture.
    ///
    /// # Errors
    ///
    /// Stderr could not be redirected.
    pub fn owning_stderr(self) -> io::Result<Self> {
        *lock(&self.stderr) = Some(OwnedStderr::take()?);
        Ok(self)
    }

    /// A handle on the delivery log (valid after the terminal is gone).
    #[must_use]
    pub fn deliveries(&self) -> Deliveries {
        self.deliveries.clone()
    }

    /// A handle on the captured output (valid after the terminal is gone).
    #[must_use]
    pub fn capture(&self) -> Capture {
        self.capture.clone()
    }
}

impl Terminal for FakeTerminal {
    type Input = FakeInput;

    fn write(&mut self, bytes: &[u8]) -> io::Result<()> {
        let began = Instant::now();
        if !self.write_delay.is_zero() {
            std::thread::sleep(self.write_delay);
        }
        self.capture.push_at(began, bytes);
        Ok(())
    }

    fn size(&self) -> io::Result<Size> {
        Ok(*self
            .size
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner))
    }

    fn input(&mut self) -> io::Result<FakeInput> {
        let script = self.script.take().ok_or_else(|| {
            io::Error::new(io::ErrorKind::AlreadyExists, "the input was already taken")
        })?;
        Ok(FakeInput::new(script, Arc::clone(&self.size)).logging(self.deliveries.clone()))
    }

    fn restorer(&self) -> io::Result<Restorer> {
        let capture = self.capture.clone();
        let stderr = Arc::clone(&self.stderr);
        Ok(Box::new(move |bytes| {
            capture.push(bytes);
            let _ = replay(&stderr, &capture);
        }))
    }

    fn replay_stray(&mut self) -> io::Result<()> {
        replay(&self.stderr, &self.capture)
    }
}

/// Hand stderr back, if it is held, and push what it collected into the
/// capture.
fn replay(stderr: &Mutex<Option<OwnedStderr>>, capture: &Capture) -> io::Result<()> {
    if let Some(owned) = lock(stderr).take() {
        let bytes = owned.restore()?;
        if !bytes.is_empty() {
            capture.push(&bytes);
        }
    }
    Ok(())
}

fn lock(stderr: &Mutex<Option<OwnedStderr>>) -> std::sync::MutexGuard<'_, Option<OwnedStderr>> {
    stderr.lock().unwrap_or_else(PoisonError::into_inner)
}

#[cfg(test)]
#[path = "fake_terminal.tests.rs"]
mod tests;
