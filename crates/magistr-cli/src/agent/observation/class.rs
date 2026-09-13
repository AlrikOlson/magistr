//! How one observed item is admitted under the channel's bounds.

/// The delivery class of an item, decided by [`Observed::class`](super::Observed::class).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ObservationClass {
    /// Delivered in order, never coalesced; overflow ends observation
    /// explicitly.
    Lossless,
    /// A delta that appends to an adjacent pending item of the same kind;
    /// overflow trims the oldest bytes behind a visible lag marker. The
    /// key names the kind (text, thought, one terminal's bytes).
    Text(u64),
    /// A state whose newest value replaces the latest pending value of
    /// its kind in place. The key names the kind (usage, plan, options).
    Telemetry(u64),
}
