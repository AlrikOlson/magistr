//! Where a container puts its children on the cross axis.

use taffy::style::AlignItems;

/// Cross-axis alignment: where a row's children sit vertically, or a
/// column's children horizontally.
///
/// The default is [`Align::Stretch`], matching CSS — a child with no
/// size on the cross axis fills its container. Anything else lets a
/// child keep its own cross size.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Align {
    /// Packed against the cross-axis start (top of a row, left of a column).
    Start,
    /// Centred on the cross axis.
    Center,
    /// Packed against the cross-axis end.
    End,
    /// Stretched to fill the container's cross axis.
    #[default]
    Stretch,
}

impl Align {
    pub(crate) const fn taffy(self) -> AlignItems {
        match self {
            Self::Start => AlignItems::START,
            Self::Center => AlignItems::CENTER,
            Self::End => AlignItems::END,
            Self::Stretch => AlignItems::STRETCH,
        }
    }
}

#[cfg(test)]
#[path = "align.tests.rs"]
mod tests;
