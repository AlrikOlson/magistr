//! How a node sizes itself along one axis.

use taffy::prelude::{Dimension, LengthPercentageAuto, auto, length, percent};

/// A size constraint along one axis.
///
/// `Percent` is a fraction of the parent's size on that axis (`0.5` is
/// half), matching CSS semantics; anything the parent cannot resolve
/// (an indefinite parent) behaves like `Auto`.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum Length {
    /// Size from content and the flex algorithm.
    #[default]
    Auto,
    /// A fixed number of cells.
    Cells(u16),
    /// A fraction of the parent's size on the same axis.
    Percent(f32),
    /// A fixed number of engine units — pixels on a window, cells on a
    /// terminal, whatever the basis says a unit is.
    ///
    /// Declared only by native branches of the view, for the geometry
    /// the design writes in pixels (a card's breathing room, a
    /// trailer's own height) that a 20 px cell cannot quantize to. The
    /// terminal's byte-identity is structural: no terminal tree carries
    /// one, and on the cell basis a unit *is* a cell, so even a stray
    /// `Px(1.0)` would resolve to exactly `Cells(1)`'s arithmetic.
    Px(f32),
}

impl Length {
    /// The length in engine units, where one cell is `unit` of them
    /// along the axis being resolved.
    ///
    /// `unit` is the axis the caller is resolving —
    /// [`Basis::x`](super::Basis::x) for a width,
    /// [`Basis::y`](super::Basis::y) for a height. A cell is 9x20 on a
    /// window, so resolving both axes with one scalar would make a
    /// square out of something that is not one; `Percent` and `Auto`
    /// are already relative and carry no unit at all.
    pub(crate) fn dimension(self, unit: f32) -> Dimension {
        match self {
            Self::Auto => auto(),
            Self::Cells(cells) => length(f32::from(cells) * unit),
            Self::Percent(fraction) => percent(fraction),
            Self::Px(units) => length(units),
        }
    }

    /// The same resolution, as the type taffy's `inset`, `min_size` and
    /// `max_size` slots take.
    pub(crate) fn length_percentage_auto(self, unit: f32) -> LengthPercentageAuto {
        match self {
            Self::Auto => auto(),
            Self::Cells(cells) => length(f32::from(cells) * unit),
            Self::Percent(fraction) => percent(fraction),
            Self::Px(units) => length(units),
        }
    }
}

#[cfg(test)]
#[path = "length.tests.rs"]
mod tests;
