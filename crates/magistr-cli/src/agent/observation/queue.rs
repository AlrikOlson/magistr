//! The shared state behind one channel: the bounded queue and its wake.

use std::collections::VecDeque;
use std::sync::{Mutex, MutexGuard, PoisonError};

use tokio::sync::Notify;

use super::capacity::ObservationCapacity;
use super::class::ObservationClass;
use super::observed::Observed;

/// One place in the queue: an item, or the lag left where text was
/// trimmed ahead of the next item.
enum Slot<T> {
    Item(T),
    Lag { bytes: usize, items: usize },
}

pub(super) struct Queue<T> {
    slots: VecDeque<Slot<T>>,
    capacity: ObservationCapacity,
    lossless_pending: usize,
    text_pending: usize,
    /// Set once the receiver closed or the lossless bound overflowed:
    /// nothing more is admitted.
    closed: bool,
    /// Set when the last sender went away: what is pending is delivered,
    /// then the receiver learns the stream ended.
    producers_gone: bool,
    /// Set when the overflow marker was admitted, for the record.
    overflowed: bool,
    /// Text bytes trimmed over the channel's life, for the record.
    trimmed_bytes: usize,
}

pub(super) struct Shared<T> {
    queue: Mutex<Queue<T>>,
    wake: Notify,
}

impl<T: Observed> Shared<T> {
    pub(super) fn new(capacity: ObservationCapacity) -> Self {
        Self {
            queue: Mutex::new(Queue {
                slots: VecDeque::new(),
                capacity,
                lossless_pending: 0,
                text_pending: 0,
                closed: false,
                producers_gone: false,
                overflowed: false,
                trimmed_bytes: 0,
            }),
            wake: Notify::new(),
        }
    }

    /// Admit `item` under the bounds. `Err` hands it back once the channel
    /// is closed — the receiver went away, or the lossless bound overflowed.
    pub(super) fn send(&self, item: T) -> Result<(), T> {
        let mut queue = self.lock();
        if queue.closed {
            return Err(item);
        }
        match item.class() {
            ObservationClass::Lossless => {
                if queue.lossless_pending >= queue.capacity.lossless_items {
                    let pending = queue.lossless_pending;
                    queue.slots.push_back(Slot::Item(T::overflowed(pending)));
                    queue.lossless_pending += 1;
                    queue.overflowed = true;
                    queue.closed = true;
                    drop(queue);
                    self.wake.notify_one();
                    return Err(item);
                }
                queue.lossless_pending += 1;
                queue.slots.push_back(Slot::Item(item));
            }
            ObservationClass::Text(key) => {
                let bytes = item.text_bytes();
                match queue.slots.back_mut() {
                    Some(Slot::Item(last)) if last.class() == ObservationClass::Text(key) => {
                        last.append_text(&item);
                    }
                    _ => queue.slots.push_back(Slot::Item(item)),
                }
                queue.text_pending += bytes;
                queue.trim_text();
            }
            ObservationClass::Telemetry(key) => {
                let latest = queue.slots.iter_mut().rev().find_map(|slot| match slot {
                    Slot::Item(pending) if pending.class() == ObservationClass::Telemetry(key) => {
                        Some(pending)
                    }
                    _ => None,
                });
                match latest {
                    Some(pending) => *pending = item,
                    None => queue.slots.push_back(Slot::Item(item)),
                }
            }
        }
        drop(queue);
        self.wake.notify_one();
        Ok(())
    }

    /// The next item, or `None` when nothing is pending. The flag says
    /// whether nothing more can ever come: the channel closed, or every
    /// sender went away.
    pub(super) fn take(&self) -> (Option<T>, bool) {
        let mut queue = self.lock();
        let closed = queue.closed || queue.producers_gone;
        let Some(slot) = queue.slots.pop_front() else {
            return (None, closed);
        };
        let item = match slot {
            Slot::Item(item) => {
                match item.class() {
                    ObservationClass::Lossless => queue.lossless_pending -= 1,
                    ObservationClass::Text(_) => queue.text_pending -= item.text_bytes(),
                    ObservationClass::Telemetry(_) => {}
                }
                item
            }
            Slot::Lag { bytes, items } => T::lagged(bytes, items),
        };
        (Some(item), closed)
    }
}

impl<T> Shared<T> {
    fn lock(&self) -> MutexGuard<'_, Queue<T>> {
        self.queue.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// The wake for the next admission or closure; enable it before
    /// looking so nothing sent in between is missed.
    pub(super) fn notified(&self) -> tokio::sync::futures::Notified<'_> {
        self.wake.notified()
    }

    /// Close from the receiver's side: nothing more is admitted, and
    /// whatever is pending is discarded.
    pub(super) fn close(&self) {
        let mut queue = self.lock();
        queue.closed = true;
        queue.slots.clear();
        queue.lossless_pending = 0;
        queue.text_pending = 0;
    }

    pub(super) fn is_closed(&self) -> bool {
        self.lock().closed
    }

    /// The last sender went: pending items still reach the receiver,
    /// and then the stream ends.
    pub(super) fn producers_gone(&self) {
        self.lock().producers_gone = true;
        self.wake.notify_one();
    }

    pub(super) fn pending_items(&self) -> usize {
        self.lock().slots.len()
    }

    pub(super) fn pending_text_bytes(&self) -> usize {
        self.lock().text_pending
    }

    pub(super) fn trimmed_bytes(&self) -> usize {
        self.lock().trimmed_bytes
    }

    pub(super) fn has_overflowed(&self) -> bool {
        self.lock().overflowed
    }
}

impl<T: Observed> Queue<T> {
    /// Bring the pending text back under its capacity by trimming the
    /// oldest bytes, and keep one lag slot where the trim happened.
    fn trim_text(&mut self) {
        while self.text_pending > self.capacity.text_bytes {
            let excess = self.text_pending - self.capacity.text_bytes;
            let Some(at) = self
                .slots
                .iter()
                .position(|slot| matches!(slot, Slot::Item(item) if matches!(item.class(), ObservationClass::Text(_))))
            else {
                return;
            };
            let Some(Slot::Item(oldest)) = self.slots.get_mut(at) else {
                return;
            };
            let went = oldest.trim_front(excess);
            if went == 0 {
                return;
            }
            self.text_pending -= went;
            self.trimmed_bytes += went;
            let emptied = oldest.text_bytes() == 0;
            let whole = usize::from(emptied);
            if emptied {
                self.slots.remove(at);
            }
            // The marker sits where the trimmed text was: an earlier one
            // for the same gap accumulates rather than multiplies.
            match at
                .checked_sub(1)
                .and_then(|before| self.slots.get_mut(before))
            {
                Some(Slot::Lag { bytes, items }) => {
                    *bytes += went;
                    *items += whole;
                }
                _ => self.slots.insert(
                    at,
                    Slot::Lag {
                        bytes: went,
                        items: whole,
                    },
                ),
            }
        }
    }
}
