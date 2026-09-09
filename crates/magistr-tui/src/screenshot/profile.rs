//! Which terminal a still is rasterized as.

use crate::graphics::{Passthrough, Protocol};

/// The terminal profile used when rendering screenshot fixtures.
/// Each profile selects the capabilities and geometry exercised by the rasterizer.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Profile {
    /// No graphics protocol: every pixel surface stands down for the
    /// cell drawing the view keeps behind it.
    #[default]
    Cells,
    /// The kitty graphics protocol, unwrapped — the pixel branches of
    /// the view, and the placements decoded back out of the frame's own
    /// bytes.
    Kitty,
}

impl Profile {
    /// The protocol to enable on the screen, if any.
    #[must_use]
    pub(crate) const fn protocol(self) -> Option<Protocol> {
        match self {
            Self::Cells => None,
            // Direct, never `Tmux`: a still is rasterized from the bytes
            // themselves, and the passthrough envelope would only be a
            // layer to peel back off.
            Self::Kitty => Some(Protocol::Kitty(Passthrough::Direct)),
        }
    }

    /// The name a golden carries for this profile — nothing for the cell
    /// path, so every still that existed before keeps its own filename.
    #[must_use]
    pub const fn suffix(self) -> &'static str {
        match self {
            Self::Cells => "",
            Self::Kitty => "-kitty",
        }
    }
}
