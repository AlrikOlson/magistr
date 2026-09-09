//! How a container distributes its children along the main axis.

use taffy::style::JustifyContent;

/// Main-axis distribution: where a row's children sit horizontally, or a
/// column's children vertically, once every child has its size.
///
/// The default is [`Justify::Start`], matching CSS — children are packed
/// against the main-axis start and the spare space falls at the end. The
/// three `Space*` forms spread the spare space between the children
/// instead, which is what a right-aligned footer item or an evenly
/// spread tab bar is describing.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Justify {
    /// Packed against the main-axis start.
    #[default]
    Start,
    /// Centred on the main axis.
    Center,
    /// Packed against the main-axis end.
    End,
    /// First and last flush with the edges, the spare space split evenly
    /// between the children.
    SpaceBetween,
    /// The spare space split evenly between the children, with half a
    /// share before the first and after the last.
    SpaceAround,
    /// The spare space split evenly between the children and at both
    /// ends, every gap the same.
    SpaceEvenly,
}

impl Justify {
    pub(crate) const fn taffy(self) -> JustifyContent {
        match self {
            Self::Start => JustifyContent::START,
            Self::Center => JustifyContent::CENTER,
            Self::End => JustifyContent::END,
            Self::SpaceBetween => JustifyContent::SPACE_BETWEEN,
            Self::SpaceAround => JustifyContent::SPACE_AROUND,
            Self::SpaceEvenly => JustifyContent::SPACE_EVENLY,
        }
    }
}

#[cfg(test)]
#[path = "justify.tests.rs"]
mod tests;
