//! The range of kitty image ids one session draws from.

use std::hash::{BuildHasher, RandomState};

use crate::graphics::{ImageId, PlacementId};

/// One session's slice of the terminal's image-id namespace.
///
/// Kitty image ids are one namespace per terminal image store, and that
/// store is not always per pane: under tmux every pane's passthrough
/// lands in the outer terminal's one store, and some terminals keep one
/// per window. Two cockpits that both count from 1 then transmit over
/// each other's images — the sparkline's placement shows the other
/// session's tech tree and vice versa. So every id carries a session
/// tag: the top [`ID_BITS`](Self::ID_BITS) − [`SLOT_BITS`](Self::SLOT_BITS)
/// bits, with [`SLOTS`](Self::SLOTS) slots of two ids below it. Everything
/// stays under 2²⁴: placement ids are 24-bit by spec, and image ids are
/// kept alike since the protocol's unicode placeholders carry them in a
/// 24-bit colour.
///
/// [`IdSpace::for_process`] is the runtime's; [`IdSpace::FIRST`] (tag 0,
/// ids from 1) is for tests that pin the wire byte for byte.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct IdSpace(u32);

impl IdSpace {
    /// Bits an id may use in all.
    pub const ID_BITS: u32 = 24;
    /// Bits of an id below the tag — the ids within one space.
    pub const SLOT_BITS: u32 = 12;
    /// How many spaces there are.
    pub const TAGS: u32 = 1 << (Self::ID_BITS - Self::SLOT_BITS);
    /// Slots one space holds — two image ids each, and the zero id is
    /// not on the wire.
    pub const SLOTS: u32 = ((1 << Self::SLOT_BITS) - 1) / 2;

    /// The space whose ids start at 1.
    pub const FIRST: Self = Self(0);

    /// The space with `tag`, wrapped into range.
    #[must_use]
    pub const fn new(tag: u32) -> Self {
        Self(tag % Self::TAGS)
    }

    /// This process's space: a tag hashed from the process id under a
    /// per-process random seed, so two sessions started together in one
    /// terminal store land on different tags with all but 1-in-`TAGS`
    /// odds.
    #[must_use]
    pub fn for_process() -> Self {
        let seed = RandomState::new().hash_one(std::process::id());
        Self::new((seed % u64::from(Self::TAGS)) as u32)
    }

    /// The session tag.
    #[must_use]
    pub const fn tag(self) -> u32 {
        self.0
    }

    /// The id below the space's first.
    const fn base(self) -> u32 {
        self.0 << Self::SLOT_BITS
    }

    /// The two image ids of slot `n`: the space's `2n+1` and `2n+2`. A
    /// slot past [`SLOTS`](Self::SLOTS) wraps to the start of the space —
    /// more surfaces than any screen has rows, so a reused id is the
    /// honest fallback rather than an id in another session's space.
    #[must_use]
    pub fn image_ids(self, n: u32) -> [ImageId; 2] {
        let n = n % Self::SLOTS;
        let id = |raw: u32| ImageId::new(self.base() + raw).expect("ids start at one");
        [id(2 * n + 1), id(2 * n + 2)]
    }

    /// The placement id of slot `n`: `n+1`. A placement is addressed
    /// together with its image id, so it needs no tag of its own.
    #[must_use]
    pub fn placement(self, n: u32) -> PlacementId {
        PlacementId::new(n % Self::SLOTS + 1).expect("placements start at one")
    }
}

#[cfg(test)]
#[path = "id_space.tests.rs"]
mod tests;
