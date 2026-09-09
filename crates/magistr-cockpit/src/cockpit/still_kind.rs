//! What kind of picture a shelf still is.

use std::path::Path;

/// The two kinds of picture the gates leave behind: a
/// still — one frame of a surface — and a filmstrip, the frames of one
/// motion side by side. The screenshot gate keeps them apart on disk
/// (`tests/golden/screens` beside `tests/golden/motion`), and the shelf
/// keeps them apart the same way, so a bundle's `stills/` and
/// `filmstrips/` say which is which without opening a file.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum StillKind {
    /// One frame of one surface.
    Still,
    /// The frames of one motion, side by side.
    Filmstrip,
}

impl StillKind {
    /// The kind a golden at `path` is: a picture under a `motion`
    /// directory is a filmstrip, anything else a still — the golden
    /// suites' own layout, which is the one fact this reads.
    #[must_use]
    pub fn of(path: &Path) -> Self {
        let in_motion = path.components().any(|part| part.as_os_str() == "motion");
        if in_motion {
            Self::Filmstrip
        } else {
            Self::Still
        }
    }

    /// The kind's word — the receipt key and the bundle directory.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Still => "still",
            Self::Filmstrip => "filmstrip",
        }
    }

    /// The bundle directory the kind's pictures are copied into.
    #[must_use]
    pub const fn dir_name(self) -> &'static str {
        match self {
            Self::Still => "stills",
            Self::Filmstrip => "filmstrips",
        }
    }
}

#[cfg(test)]
#[path = "still_kind.tests.rs"]
mod tests;
