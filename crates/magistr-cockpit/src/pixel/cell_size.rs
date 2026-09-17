//! One terminal cell in pixels: what a surface's size in cells scales by.

/// One cell's `(width, height)` in pixels.
///
/// The surface supplies measurements; shared geometry uses a default until then.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct CellSize {
    /// Cell width in pixels.
    pub width: u16,
    /// Cell height in pixels.
    pub height: u16,
}

impl CellSize {
    /// The guess when the terminal answered neither query: 10×20 px, a
    /// common size for a 12–13 pt monospace font at 1× scale. Placements
    /// are scaled into their cells by the terminal, so a wrong guess costs
    /// sharpness, not layout.
    pub const DEFAULT: Self = Self {
        width: 10,
        height: 20,
    };

    /// A cell size from its pixels.
    #[must_use]
    pub const fn new(width: u16, height: u16) -> Self {
        Self { width, height }
    }

    /// The pixel size of `cols × rows` cells, saturating at the coordinate limit.
    #[must_use]
    pub fn pixels(self, cols: u16, rows: u16) -> (u16, u16) {
        let width = u32::from(cols) * u32::from(self.width);
        let height = u32::from(rows) * u32::from(self.height);
        (
            u16::try_from(width).unwrap_or(u16::MAX),
            u16::try_from(height).unwrap_or(u16::MAX),
        )
    }
}

#[cfg(test)]
#[path = "cell_size.tests.rs"]
mod tests;
