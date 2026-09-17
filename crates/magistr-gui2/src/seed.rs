//! A byte stream derived from a name — the one seed every derived mark
//! (a project's sigil, its cluster on the Galaxy) draws from.

/// A stream of bytes seeded by a name, the same on every machine and
/// every toolchain.
///
/// FNV-1a over the trimmed, lowercased name, then SplitMix64's mixing
/// step per draw. Both are written out rather than borrowed from `std`:
/// `DefaultHasher` is explicitly not stable across Rust releases, and a
/// mark that changed shape when the toolchain moved would be a
/// different project's mark on the same project. The case and space
/// folding is deliberate too — `Magistr ` and `magistr` are one project
/// to a human reading a breadcrumb, so they draw one stream.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Seed {
    state: u64,
}

impl Seed {
    /// The stream for `name`.
    #[must_use]
    pub fn of(name: &str) -> Self {
        Self {
            state: fnv1a(name.trim().to_lowercase().as_bytes()),
        }
    }

    /// The next byte, re-mixed so consecutive draws are independent
    /// rather than eight slices of the same number.
    pub fn byte(&mut self) -> u8 {
        self.state = self.state.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        ((z ^ (z >> 31)) >> 24) as u8
    }

    /// The next byte as a share, `0.0..=1.0`.
    pub fn unit(&mut self) -> f32 {
        f32::from(self.byte()) / 255.0
    }

    /// Fisher–Yates over the stream — the standard shuffle, so every
    /// ordering is equally likely and the bias a plain modulo would add
    /// is the only thing left out.
    pub fn shuffle<T>(&mut self, items: &mut [T]) {
        for index in (1..items.len()).rev() {
            let pick = usize::from(self.byte()) % (index + 1);
            items.swap(index, pick);
        }
    }
}

/// FNV-1a, 64-bit.
fn fnv1a(bytes: &[u8]) -> u64 {
    const OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
    const PRIME: u64 = 0x0000_0100_0000_01b3;
    bytes.iter().fold(OFFSET, |hash, byte| {
        (hash ^ u64::from(*byte)).wrapping_mul(PRIME)
    })
}

#[cfg(test)]
#[path = "seed.tests.rs"]
mod tests;
