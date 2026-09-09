//! How a surface's pixel bytes are packed for the wire.

use std::io::Write as _;

/// The transport encoding of a transmitted image (the kitty `o` key).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Compression {
    /// Raw pixel bytes, base64 only.
    None,
    /// A zlib (RFC 1950) stream at this deflate level (1 = fastest, 9 =
    /// smallest); `o=z` on the wire.
    Zlib(u32),
}

impl Compression {
    /// What the compositor uses unless told otherwise: zlib at level 1,
    /// measured in `tests/pixel_perf.rs` on a 640×396 px
    /// sparkline — 1 017 087 → 26 379 bytes per dirty frame (38×) for
    /// 454 → 547 µs of encoding; level 6 halves the bytes again at 2.7×
    /// the time, level 9 is a rounding error over 6 at twice its cost.
    pub const DEFAULT: Self = Self::Zlib(1);

    /// The `o=` key this encoding adds to a transmit, if any.
    #[must_use]
    pub const fn key(self) -> Option<&'static str> {
        match self {
            Self::None => None,
            Self::Zlib(_) => Some("o=z"),
        }
    }

    /// Replace `out` with the wire payload for `raw`. Returns whether the
    /// payload is a zlib stream (`o=z`).
    pub fn apply(self, raw: &[u8], out: &mut Vec<u8>) -> bool {
        out.clear();
        match self {
            Self::None => {
                out.extend_from_slice(raw);
                false
            }
            Self::Zlib(level) => {
                let buffer = std::mem::take(out);
                let mut encoder =
                    flate2::write::ZlibEncoder::new(buffer, flate2::Compression::new(level));
                // Writing into a Vec cannot fail; a failure here would be a
                // bug in the encoder, and an empty payload is the honest
                // result of one.
                let _ = encoder.write_all(raw);
                *out = encoder.finish().unwrap_or_default();
                true
            }
        }
    }
}

#[cfg(test)]
#[path = "compression.tests.rs"]
mod tests;
