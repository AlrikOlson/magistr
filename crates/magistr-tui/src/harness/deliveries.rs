//! When a fake terminal handed bytes to the reader.

use std::sync::{Arc, Mutex};
use std::time::Instant;

/// A shared log of the instants at which the fake input delivered each
/// scripted byte step — the "input arrived" end of an input-to-paint
/// measurement (the [`Capture`](super::Capture) holds the other end).
#[derive(Clone, Debug, Default)]
pub struct Deliveries {
    instants: Arc<Mutex<Vec<Instant>>>,
}

impl Deliveries {
    /// An empty log.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Record a delivery now.
    pub fn record(&self) {
        self.lock().push(Instant::now());
    }

    /// Every delivery so far, in order.
    #[must_use]
    pub fn instants(&self) -> Vec<Instant> {
        self.lock().clone()
    }

    /// Deliveries so far.
    #[must_use]
    pub fn len(&self) -> usize {
        self.lock().len()
    }

    /// Whether nothing was delivered.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.lock().is_empty()
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Vec<Instant>> {
        self.instants
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}
