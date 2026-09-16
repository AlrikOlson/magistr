//! Everything a fake terminal was told.

use std::sync::{Arc, Mutex};
use std::time::Instant;

/// A shared byte sink: the [`FakeTerminal`](super::FakeTerminal) appends
/// every write, the test reads the whole stream back — during the run or
/// after the runtime has consumed the terminal — and the instant of every
/// write, for input-to-paint measurements.
#[derive(Clone, Debug, Default)]
pub struct Capture {
    bytes: Arc<Mutex<Vec<u8>>>,
    writes: Arc<Mutex<Vec<Write>>>,
}

/// One write: when it began, when it was over, how many bytes.
#[derive(Clone, Copy, Debug)]
struct Write {
    began: Instant,
    ended: Instant,
    len: usize,
}

impl Capture {
    /// An empty capture.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Append `bytes`, noting when.
    pub fn push(&self, bytes: &[u8]) {
        self.push_at(Instant::now(), bytes);
    }

    /// Append `bytes` as a write that began at `when` — what a terminal
    /// that takes its time over a write reports, so a message that
    /// arrived while the write was in flight is dated after it.
    pub fn push_at(&self, when: Instant, bytes: &[u8]) {
        self.lock().extend_from_slice(bytes);
        self.writes
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(Write {
                began: when,
                ended: Instant::now(),
                len: bytes.len(),
            });
    }

    /// Every write so far: when it began and how many bytes it was.
    #[must_use]
    pub fn writes(&self) -> Vec<(Instant, usize)> {
        self.spans()
            .into_iter()
            .map(|(began, _, len)| (began, len))
            .collect()
    }

    /// Every write so far as a span — when it began, when it was over,
    /// how many bytes — so a test can tell which write a message arrived
    /// *during*, and therefore was already queued when the loop came
    /// back from it.
    #[must_use]
    pub fn spans(&self) -> Vec<(Instant, Instant, usize)> {
        self.writes
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .iter()
            .map(|w| (w.began, w.ended, w.len))
            .collect()
    }

    /// A copy of everything written so far.
    #[must_use]
    pub fn bytes(&self) -> Vec<u8> {
        self.lock().clone()
    }

    /// Bytes written so far.
    #[must_use]
    pub fn len(&self) -> usize {
        self.lock().len()
    }

    /// Whether nothing was written.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.lock().is_empty()
    }

    /// How many times `needle` occurs in the stream.
    #[must_use]
    pub fn count(&self, needle: &[u8]) -> usize {
        if needle.is_empty() {
            return 0;
        }
        self.lock()
            .windows(needle.len())
            .filter(|w| *w == needle)
            .count()
    }

    /// Decode the actual graphics on screen before a write boundary.
    #[must_use]
    pub fn pixels_before(&self, until: Instant) -> Vec<crate::screenshot::Placement> {
        let end: usize = self
            .writes()
            .iter()
            .take_while(|(at, _)| *at < until)
            .map(|(_, len)| len)
            .sum();
        crate::screenshot::decode(&self.bytes()[..end])
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Vec<u8>> {
        // A poisoned capture still holds the bytes written before the
        // panic; those are what a test wants to see.
        self.bytes
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}
