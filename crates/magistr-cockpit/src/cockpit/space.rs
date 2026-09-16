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
    /// Outer inspector width and minimum stage text width for side-by-side panes.
    pub const INSPECTOR_COLS: u16 = 18 * Self::GAP;
    pub const STAGE_COLS: u16 = 20 * Self::GAP;
    /// A stacked card keeps its four header rows and one field inside its ring.
    pub const INSPECTOR_ROWS: u16 = 7 * Self::TIGHT;
}

#[cfg(test)]
#[path = "space.tests.rs"]
mod tests;
