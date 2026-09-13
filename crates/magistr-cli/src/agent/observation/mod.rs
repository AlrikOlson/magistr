//! A bounded, classified observation channel from a session owner to its
//! one observer.
//!
//! The resident session owner emits everything the provider says; the
//! observer — the cockpit relay or a headless drain — may be slower than
//! the provider streams. Three classes keep that gap bounded without ever
//! losing a fact silently:
//!
//! * **Lossless** items — decisions, terminal outcomes, tool events,
//!   errors — are delivered in order under
//!   [`ObservationCapacity::lossless_items`]. Overflow admits one terminal
//!   [`Observed::overflowed`] marker and then closes the channel to the
//!   owner, which detaches the observer; the provider stays resident.
//! * **Text** items — adjacent deltas of the same kind — coalesce into one
//!   pending item under [`ObservationCapacity::text_bytes`]. Overflow
//!   trims the oldest pending text and leaves one visible
//!   [`Observed::lagged`] marker where the trim happened.
//! * **Telemetry** items — usage, plans, option sets — replace the latest
//!   pending item of their kind in place: the newest values are what the
//!   observer sees, never a backlog of stale ones.
//!
//! Ordering within the lossless class is never changed; a text item never
//! crosses a lossless item; a telemetry replacement keeps the position of
//! the item it replaces.

pub mod capacity;
pub mod class;
pub mod observed;
pub mod receiver;
pub mod sender;

mod agent_event;
mod agent_message;
mod queue;

pub use capacity::ObservationCapacity;
pub use class::ObservationClass;
pub use observed::Observed;
pub use receiver::ObservationReceiver;
pub use sender::ObservationSender;

use std::sync::Arc;

/// A fresh channel under `capacity`: the owner's sender and the observer's
/// receiver.
#[must_use]
pub fn channel<T: Observed>(
    capacity: ObservationCapacity,
) -> (ObservationSender<T>, ObservationReceiver<T>) {
    let shared = Arc::new(queue::Shared::new(capacity));
    (
        ObservationSender::new(Arc::clone(&shared)),
        ObservationReceiver::new(shared),
    )
}

#[cfg(test)]
#[path = "../observation.tests.rs"]
mod tests;
