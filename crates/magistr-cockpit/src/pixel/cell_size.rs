//! One terminal cell in pixels: what a surface's size in cells scales by.

/// One cell's `(width, height)` in pixels.
///
/// The terminal is asked at startup (`CSI 16 t`, answered as
/// [`Reply::CellPixels`](crate::input::Reply::CellPixels)); a terminal that
/// only answers `CSI 14 t` gives the text area in pixels, divided here by
/// the size in cells; one that answers neither gets [`CellSize::DEFAULT`].
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

    /// What the terminal established: the direct `CSI 16 t` answer when
    /// there is one, else the `CSI 14 t` text area divided by `cols × rows`.
    /// `None` when neither answered, a division would be by zero, or an
    /// answer would make a zero-pixel cell.
    #[must_use]
    pub fn measured(
        cell: Option<(u16, u16)>,
        window: Option<(u16, u16)>,
        cols: u16,
        rows: u16,
    ) -> Option<Self> {
        let (width, height) = cell.or_else(|| {
            let (width, height) = window?;
            (cols > 0 && rows > 0).then(|| (width / cols, height / rows))
        })?;
        (width > 0 && height > 0).then_some(Self { width, height })
    }

    /// [`CellSize::measured`], falling back to [`CellSize::DEFAULT`].
    #[must_use]
    pub fn resolve(
        cell: Option<(u16, u16)>,
        window: Option<(u16, u16)>,
        cols: u16,
        rows: u16,
    ) -> Self {
        Self::measured(cell, window, cols, rows).unwrap_or(Self::DEFAULT)
    }

    /// The pixel size of `cols × rows` cells, saturating at what the kitty
    /// `s`/`v` keys carry.
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
