//! What the cockpit knows about its own view of the store.

use magistr_engine::{Delivery, FeedStatus};

use super::freshness::Freshness;

/// The projection's bookkeeping: how fresh what the model holds is, and
/// how the last event was delivered — folded from the same
/// [`Msg::Engine`](super::Msg::Engine) messages the views fold, so it
/// cannot disagree with them.
///
/// It decides one thing for the views: a duplicate is not folded, so a
/// frame delivered twice counts once. Everything else it only records.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Projection {
    freshness: Freshness,
    last_delivery: Option<Delivery>,
}

impl Projection {
    /// How much the model can be trusted right now.
    #[must_use]
    pub const fn freshness(&self) -> Freshness {
        self.freshness
    }

    /// How the last event was delivered, once one was.
    #[must_use]
    pub const fn last_delivery(&self) -> Option<Delivery> {
        self.last_delivery
    }

    /// A snapshot landed: the model is the store again.
    pub const fn seeded(&mut self) {
        self.freshness = Freshness::Current;
    }

    /// An event arrived with `delivery`; whether the views should fold it.
    pub const fn delivered(&mut self, delivery: Delivery) -> bool {
        self.last_delivery = Some(delivery);
        if delivery.is_duplicate() {
            return false;
        }
        if delivery.needs_snapshot() {
            self.freshness = Freshness::Stale;
        } else if matches!(self.freshness, Freshness::Unavailable) {
            self.freshness = Freshness::Current;
        }
        true
    }

    /// The feed's attachment changed: a hangup means frames may be lost
    /// until the next snapshot; nothing else moves the freshness — a
    /// reattachment is not a guarantee, the snapshot it asks for is.
    pub const fn feed(&mut self, status: FeedStatus) {
        if matches!(status, FeedStatus::Disconnected)
            && !matches!(self.freshness, Freshness::Unavailable)
        {
            self.freshness = Freshness::Stale;
        }
    }
}

#[cfg(test)]
#[path = "projection.tests.rs"]
mod tests;
