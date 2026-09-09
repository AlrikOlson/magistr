//! How many layout units one cell is worth on each axis.

/// The unit the layout engine measures in, expressed as the size of one
/// cell along each axis.
///
/// # Why one engine and not two
///
/// `view(model) -> Node` is *shared*: the terminal and the window lay
/// out the same tree, and a second pixel-native layout path would be a
/// second derivation of it — exactly the drift
/// [`Surface`](crate::cockpit::Surface) exists to prevent. So the tree
/// keeps its cell vocabulary ([`Length::Cells`](super::Length::Cells))
/// and the *engine* is told what a cell is worth: [`Basis::CELLS`] on a
/// terminal, the window's real cell metrics on a native surface, where
/// a cell is 9x20 and one scalar therefore cannot serve both axes.
///
/// That makes the terminal's byte-identity **arithmetic rather than
/// empirical**: every length is multiplied by `1.0` and every rect is
/// divided by `1.0` before it is rounded, which is the code that ran
/// before the basis existed. No golden has to agree for the guarantee
/// to hold.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Basis {
    x: f32,
    y: f32,
}

impl Basis {
    /// One unit per cell on both axes: the terminal's basis, and the
    /// identity of every conversion in this module.
    pub const CELLS: Self = Self { x: 1.0, y: 1.0 };

    /// A basis where one cell is `x` units wide and `y` units tall.
    ///
    /// Both axes are floored at `1.0`: a zero or negative cell would
    /// make every rect a division by zero, and a window mid-resize
    /// legitimately reports sizes a naive metric would round to zero.
    #[must_use]
    pub fn new(x: f32, y: f32) -> Self {
        Self {
            x: Self::axis(x),
            y: Self::axis(y),
        }
    }

    fn axis(value: f32) -> f32 {
        if value.is_finite() {
            value.max(1.0)
        } else {
            1.0
        }
    }

    /// Units one cell spans horizontally.
    #[must_use]
    pub const fn x(self) -> f32 {
        self.x
    }

    /// Units one cell spans vertically.
    #[must_use]
    pub const fn y(self) -> f32 {
        self.y
    }

    /// Whether this is [`Basis::CELLS`] — the case in which every
    /// conversion here is the identity.
    #[must_use]
    pub fn is_cells(self) -> bool {
        self == Self::CELLS
    }

    /// `cells` cells as units along the horizontal axis.
    #[must_use]
    pub fn across(self, cells: u16) -> f32 {
        f32::from(cells) * self.x
    }

    /// `cells` cells as units along the vertical axis.
    #[must_use]
    pub fn down(self, cells: u16) -> f32 {
        f32::from(cells) * self.y
    }
}

impl Default for Basis {
    fn default() -> Self {
        Self::CELLS
    }
}

#[cfg(test)]
#[path = "basis.tests.rs"]
mod tests;
