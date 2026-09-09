//! The input side of a fake terminal: the script, played on the clock.

use std::collections::VecDeque;
use std::io;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use super::deliveries::Deliveries;
use super::script::Script;
use super::step::Step;
use crate::term::TerminalInput;
use magistr_cockpit::cockpit::Size;

/// Plays a [`Script`]: a read returns the next due bytes, waits (up to
/// its timeout) for the next step to come due, reports a resize as a
/// timeout after updating the shared size, and answers end of file once
/// the script closed the input. The clock starts at the first read.
#[derive(Debug)]
pub struct FakeInput {
    steps: VecDeque<(Duration, Step)>,
    size: Arc<Mutex<Size>>,
    start: Option<Instant>,
    pending: Vec<u8>,
    closed: bool,
    deliveries: Deliveries,
}

impl FakeInput {
    /// Play `script`, updating `size` on its resize steps.
    #[must_use]
    pub fn new(script: Script, size: Arc<Mutex<Size>>) -> Self {
        Self {
            steps: script.into_steps().into(),
            size,
            start: None,
            pending: Vec::new(),
            closed: false,
            deliveries: Deliveries::new(),
        }
    }

    /// Log the instant of every byte delivery in `deliveries`.
    #[must_use]
    pub fn logging(mut self, deliveries: Deliveries) -> Self {
        self.deliveries = deliveries;
        self
    }

    /// Steps not yet delivered.
    #[must_use]
    pub fn remaining(&self) -> usize {
        self.steps.len()
    }

    fn take_pending(&mut self, buf: &mut [u8]) -> usize {
        let n = self.pending.len().min(buf.len());
        buf[..n].copy_from_slice(&self.pending[..n]);
        self.pending.drain(..n);
        n
    }

    fn apply(&mut self, step: Step) -> Option<Option<usize>> {
        match step {
            Step::Bytes(bytes) => {
                self.pending = bytes;
                self.deliveries.record();
                None
            }
            Step::Resize(size) => {
                *self
                    .size
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner) = size;
                Some(None)
            }
            Step::Close => {
                self.closed = true;
                Some(Some(0))
            }
        }
    }
}

impl TerminalInput for FakeInput {
    fn read(&mut self, buf: &mut [u8], timeout: Duration) -> io::Result<Option<usize>> {
        let start = *self.start.get_or_insert_with(Instant::now);
        let deadline = Instant::now() + timeout;
        loop {
            if !self.pending.is_empty() {
                return Ok(Some(self.take_pending(buf)));
            }
            if self.closed {
                return Ok(Some(0));
            }
            let now = Instant::now();
            let Some(&(at, _)) = self.steps.front() else {
                std::thread::sleep(deadline.saturating_duration_since(now));
                return Ok(None);
            };
            let due = start + at;
            if due <= now {
                let (_, step) = self.steps.pop_front().expect("front was there");
                if let Some(answer) = self.apply(step) {
                    return Ok(answer);
                }
                continue;
            }
            if now >= deadline {
                return Ok(None);
            }
            std::thread::sleep(due.min(deadline) - now);
        }
    }

    fn size(&self) -> io::Result<Size> {
        Ok(*self
            .size
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner))
    }
}

#[cfg(test)]
#[path = "fake_input.tests.rs"]
mod tests;
