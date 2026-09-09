//! An axis-aligned rectangle of cells.

/// A rectangle in cell coordinates: `x`/`y` are the top-left cell, `width`
/// and `height` count cells. Every edge is inclusive on the top-left and
/// exclusive on the bottom-right, so a rect never overflows `u16`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Rect {
    /// Leftmost column.
    pub x: u16,
    /// Topmost row.
    pub y: u16,
    /// Columns covered.
    pub width: u16,
    /// Rows covered.
    pub height: u16,
}

impl Rect {
    /// A rect from its top-left cell and size.
    #[must_use]
    pub const fn new(x: u16, y: u16, width: u16, height: u16) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    /// One past the rightmost column (saturating).
    #[must_use]
    pub const fn right(self) -> u16 {
        self.x.saturating_add(self.width)
    }

    /// One past the bottom row (saturating).
    #[must_use]
    pub const fn bottom(self) -> u16 {
        self.y.saturating_add(self.height)
    }

    /// Whether the rect covers no cell at all.
    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.width == 0 || self.height == 0
    }

    /// Whether `(col, row)` lies inside the rect.
    #[must_use]
    pub const fn contains(self, col: u16, row: u16) -> bool {
        col >= self.x && col < self.right() && row >= self.y && row < self.bottom()
    }

    /// The overlap of two rects, or an empty rect at the origin when they
    /// do not overlap.
    #[must_use]
    pub fn intersection(self, other: Self) -> Self {
        let x = self.x.max(other.x);
        let y = self.y.max(other.y);
        let right = self.right().min(other.right());
        let bottom = self.bottom().min(other.bottom());
        if right <= x || bottom <= y {
            return Self::default();
        }
        Self::new(x, y, right - x, bottom - y)
    }
}

#[cfg(test)]
#[path = "rect.tests.rs"]
mod tests;
