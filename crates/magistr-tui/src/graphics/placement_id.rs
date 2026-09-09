//! The identity of one placement of a stored image.

use std::num::NonZeroU32;

/// A kitty placement id (`p=`): one on-screen instance of an image, so it
/// can be deleted or replaced without touching the image data. Zero is
/// "no placement id" on the wire and is not representable.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct PlacementId(NonZeroU32);

impl PlacementId {
    /// Wrap a raw id; `None` for zero.
    #[must_use]
    pub const fn new(raw: u32) -> Option<Self> {
        match NonZeroU32::new(raw) {
            Some(id) => Some(Self(id)),
            None => None,
        }
    }

    /// The raw id as written on the wire.
    #[must_use]
    pub const fn raw(self) -> u32 {
        self.0.get()
    }
}

#[cfg(test)]
#[path = "placement_id.tests.rs"]
mod tests;
