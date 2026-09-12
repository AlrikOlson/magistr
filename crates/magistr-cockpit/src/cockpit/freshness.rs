//! How much the projection can be trusted right now.

/// Whether what the cockpit holds of the store is the store.
///
/// Three states, and only the store or the feed move between them: a
/// snapshot makes the projection current; a gap in the feed's sequence, a
/// new owner behind it, or the feed hanging up makes it stale until the
/// next snapshot lands; before any snapshot or event at all there is
/// nothing to trust or distrust.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Freshness {
    /// Nothing has been read yet: no snapshot, no event.
    #[default]
    Unavailable,
    /// Every change the store made is here, as far as the feed can tell.
    Current,
    /// Changes may be missing: the feed skipped, changed hands or hung
    /// up, and no snapshot has landed since.
    Stale,
}
