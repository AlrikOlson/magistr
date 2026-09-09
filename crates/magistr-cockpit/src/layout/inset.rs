//! How far an overlay sits from its parent's edges.

use taffy::geometry::Rect as TaffyRect;
use taffy::prelude::LengthPercentageAuto;

use super::basis::Basis;
use super::length::Length;

/// The distance an out-of-flow node keeps from each of its parent's four
/// edges.
///
/// Every edge is a [`Length`]; [`Length::Auto`] leaves that edge
/// unconstrained, so the node keeps its own size and sits where the flex
/// algorithm would have put it. Setting *both* edges of an axis without
/// also setting a size on that axis stretches the node between them —
/// CSS's `left: 0; right: 0` — which is what makes [`Inset::all(0)`] a
/// layer covering its parent.
///
/// Built by chaining from [`Inset::none`]:
/// `Inset::none().top(Length::Cells(1)).right(Length::Cells(2))`.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Inset {
    /// Distance from the parent's left edge.
    pub left: Length,
    /// Distance from the parent's right edge.
    pub right: Length,
    /// Distance from the parent's top edge.
    pub top: Length,
    /// Distance from the parent's bottom edge.
    pub bottom: Length,
}

impl Inset {
    /// Every edge unconstrained: the node keeps its own size and its
    /// in-flow position, but takes no space from its siblings.
    #[must_use]
    pub const fn none() -> Self {
        Self {
            left: Length::Auto,
            right: Length::Auto,
            top: Length::Auto,
            bottom: Length::Auto,
        }
    }

    /// The same distance from all four edges — a layer covering its
    /// parent, inset by `cells` on every side.
    #[must_use]
    pub const fn all(cells: u16) -> Self {
        Self {
            left: Length::Cells(cells),
            right: Length::Cells(cells),
            top: Length::Cells(cells),
            bottom: Length::Cells(cells),
        }
    }

    /// Distance from the parent's left edge.
    #[must_use]
    pub const fn left(mut self, left: Length) -> Self {
        self.left = left;
        self
    }

    /// Distance from the parent's right edge.
    #[must_use]
    pub const fn right(mut self, right: Length) -> Self {
        self.right = right;
        self
    }

    /// Distance from the parent's top edge.
    #[must_use]
    pub const fn top(mut self, top: Length) -> Self {
        self.top = top;
        self
    }

    /// Distance from the parent's bottom edge.
    #[must_use]
    pub const fn bottom(mut self, bottom: Length) -> Self {
        self.bottom = bottom;
        self
    }

    /// The four edges in engine units: the horizontal pair on the
    /// basis's x axis and the vertical pair on its y, because an inset
    /// of one cell is a different number of units left and up.
    pub(crate) fn taffy(self, basis: Basis) -> TaffyRect<LengthPercentageAuto> {
        TaffyRect {
            left: self.left.length_percentage_auto(basis.x()),
            right: self.right.length_percentage_auto(basis.x()),
            top: self.top.length_percentage_auto(basis.y()),
            bottom: self.bottom.length_percentage_auto(basis.y()),
        }
    }
}

#[cfg(test)]
#[path = "inset.tests.rs"]
mod tests;
