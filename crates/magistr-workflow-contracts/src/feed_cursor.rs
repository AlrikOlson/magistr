//! The subscriber's side of the sequence: classifying each arrival.

use super::delivery::Delivery;
use super::event_cursor::EventCursor;

/// The last cursor a subscriber folded, and the rule that turns the next
/// frame's cursor into a [`Delivery`].
///
/// Kept across reconnects on purpose: the frames a publisher emitted while
/// this subscriber was detached are gone (the socket buffers nothing), and
/// the first frame after reattaching is what reveals them — as a `Gap`
/// against the cursor kept from before, which a fresh cursor would have
/// read as an innocent `First`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct FeedCursor {
    last: Option<EventCursor>,
}

impl FeedCursor {
    /// A cursor that has seen nothing.
    #[must_use]
    pub const fn new() -> Self {
        Self { last: None }
    }

    /// The cursor of the last sequenced frame folded, if any.
    #[must_use]
    pub const fn last(&self) -> Option<EventCursor> {
        self.last
    }

    /// Classify a frame carrying `cursor` and, unless it is a duplicate,
    /// advance to it. `None` is a frame from a publisher that stamps
    /// nothing and leaves the cursor where it was.
    pub fn accept(&mut self, cursor: Option<EventCursor>) -> Delivery {
        let Some(cursor) = cursor else {
            return Delivery::Unsequenced;
        };
        let delivery = match self.last {
            None => Delivery::First,
            Some(last) if last.epoch != cursor.epoch => Delivery::NewEpoch,
            Some(last) if cursor.sequence <= last.sequence => Delivery::Duplicate,
            Some(last) if cursor.sequence == last.sequence + 1 => Delivery::InOrder,
            Some(last) => Delivery::Gap {
                missed: cursor.sequence - last.sequence - 1,
            },
        };
        if !delivery.is_duplicate() {
            self.last = Some(cursor);
        }
        delivery
    }
}
