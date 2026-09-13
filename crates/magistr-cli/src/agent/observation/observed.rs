//! What the channel needs to know about the items it carries.

use super::class::ObservationClass;

/// An item vocabulary the bounded channel can classify, coalesce and
/// mark. Implemented for the headless message stream and the cockpit's
/// event stream; the channel itself knows neither.
pub trait Observed: Send + 'static {
    /// The item's delivery class.
    fn class(&self) -> ObservationClass;

    /// The bytes this item holds against the text capacity — zero for
    /// anything that is not [`ObservationClass::Text`].
    fn text_bytes(&self) -> usize;

    /// Append `next`'s text to this item's. Called only when both are
    /// [`ObservationClass::Text`] with the same key.
    fn append_text(&mut self, next: &Self);

    /// Drop up to `bytes` from the front of this item's text, on a char
    /// boundary; answers how many bytes went. Called only on a
    /// [`ObservationClass::Text`] item.
    fn trim_front(&mut self, bytes: usize) -> usize;

    /// The visible marker for text the observer never saw: `bytes` of
    /// `items` deltas trimmed ahead of the item that follows.
    fn lagged(bytes: usize, items: usize) -> Self;

    /// The terminal marker admitted when the lossless backlog reached
    /// its capacity with `pending` items waiting: observation ends after
    /// it.
    fn overflowed(pending: usize) -> Self;
}
