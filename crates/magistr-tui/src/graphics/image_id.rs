//! The identity of an image the terminal has stored.

use std::num::NonZeroU32;

/// A kitty image id (`i=`): the handle for one transmitted image. Zero
/// means "no id" on the wire, so it is not representable.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ImageId(NonZeroU32);

impl ImageId {
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
#[path = "image_id.tests.rs"]
mod tests;
