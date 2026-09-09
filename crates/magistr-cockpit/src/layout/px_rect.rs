//! An axis-aligned rectangle in the engine's own units.

use super::basis::Basis;
use super::rect::Rect;

/// A rectangle in layout units, exactly as the flex engine placed it —
/// no rounding, no `u16`.
///
/// A "unit" is whatever [`Basis`] said a cell is worth: on
/// [`Basis::CELLS`] a unit *is* a cell and this rect carries the same
/// numbers [`Rect`] does, and on a window's basis it is a physical
/// pixel. That is why the native painter, the pixel chrome and the
/// accessibility bounds read this and the cell renderer reads [`Rect`]:
/// one layout, two readings, rather than two layouts.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PxRect {
    /// Left edge.
    pub x: f32,
    /// Top edge.
    pub y: f32,
    /// Units covered horizontally.
    pub width: f32,
    /// Units covered vertically.
    pub height: f32,
}

impl PxRect {
    /// A rect from its top-left corner and size. A negative extent is
    /// clamped to zero, so `right`/`bottom` are never left of `x`/`y`.
    #[must_use]
    pub fn new(x: f32, y: f32, width: f32, height: f32) -> Self {
        Self {
            x,
            y,
            width: width.max(0.0),
            height: height.max(0.0),
        }
    }

    /// The right edge.
    #[must_use]
    pub fn right(self) -> f32 {
        self.x + self.width
    }

    /// The bottom edge.
    #[must_use]
    pub fn bottom(self) -> f32 {
        self.y + self.height
    }

    /// Whether the rect covers nothing.
    #[must_use]
    pub fn is_empty(self) -> bool {
        self.width <= 0.0 || self.height <= 0.0
    }

    /// Whether `(x, y)` lies inside the rect — left and top inclusive,
    /// right and bottom exclusive, so two abutting rects never both
    /// claim the boundary.
    #[must_use]
    pub fn contains(self, x: f64, y: f64) -> bool {
        x >= f64::from(self.x)
            && x < f64::from(self.right())
            && y >= f64::from(self.y)
            && y < f64::from(self.bottom())
    }

    /// The overlap of two rects, or an empty rect at the origin when
    /// they do not overlap.
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

    /// The whole cells this rect covers on `basis`.
    ///
    /// The edges are rounded, not the size, so two abutting rects stay
    /// abutting — and on [`Basis::CELLS`] the division is by `1.0`, so
    /// this is exactly the arithmetic the cell-only engine did before
    /// the basis existed.
    #[must_use]
    pub fn to_cells(self, basis: Basis) -> Rect {
        let left = (self.x / basis.x()).round().max(0.0);
        let top = (self.y / basis.y()).round().max(0.0);
        let right = (self.right() / basis.x()).round().max(left);
        let bottom = (self.bottom() / basis.y()).round().max(top);
        Rect::new(
            cell(left),
            cell(top),
            cell(right - left),
            cell(bottom - top),
        )
    }
}

/// Clamped, so the truncating cast is exact for every finite value and
/// NaN becomes 0.
fn cell(value: f32) -> u16 {
    value.clamp(0.0, f32::from(u16::MAX)) as u16
}

#[cfg(test)]
#[path = "px_rect.tests.rs"]
mod tests;
