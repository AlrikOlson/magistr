//! One cell that differs between two frames.

use magistr_cockpit::render::Cell;

/// A cell of the next frame that must be redrawn.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Change<'a> {
    /// Column, 0-based.
    pub col: u16,
    /// Row, 0-based.
    pub row: u16,
    /// The cell to draw.
    pub cell: &'a Cell,
}
