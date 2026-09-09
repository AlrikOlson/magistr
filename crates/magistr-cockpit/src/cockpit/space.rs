//! The cockpit's spacing scale.

/// Every gap and pad in the view layer draws from this three-step scale,
/// so density is one decision instead of scattered magic numbers:
/// mission-control dense, a cell only where it separates.
pub struct Space;

impl Space {
    /// Between tight siblings: a run of LEDs, a card's buttons.
    pub const TIGHT: u16 = 1;
    /// Between labelled groups: tabs, the strip's clusters, the
    /// launcher's actions.
    pub const GAP: u16 = 2;
    /// Inside an unframed surface's edge (framed panels inset through
    /// their ring instead).
    pub const PAD: u16 = 1;
}

#[cfg(test)]
#[path = "space.tests.rs"]
mod tests;
