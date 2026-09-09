//! One picture on a shipped chunk's shelf.

use std::path::PathBuf;

use super::still_kind::StillKind;
use crate::turn::StreamImage;

/// A still or filmstrip the shipped chunk left behind — one of the
/// gates' goldens the chunk's commit re-blessed — decoded once, so the
/// gallery draws it and the bundle copies it from the same record.
///
/// Data only: the read that fills one is
/// [`shelf::load_still`](crate::shelf::load_still), an effect the entry
/// point's job runs; the core never opens a file.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ShelfStill {
    /// The file's own name — `roadmap-mocha-80x24.png`.
    pub name: String,
    /// Where the golden is, absolute, so the bundle can copy it.
    pub source: PathBuf,
    /// Still or filmstrip, by the golden suite's layout.
    pub kind: StillKind,
    /// The decoded picture.
    pub image: StreamImage,
}

impl ShelfStill {
    /// The picture's `(width, height)` in pixels.
    #[must_use]
    pub fn size(&self) -> (u32, u32) {
        (
            u32::from(self.image.width()),
            u32::from(self.image.height()),
        )
    }
}
