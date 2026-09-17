use crate::icon::Icon;
use crate::shape::Shape;
use crate::sigil::Sigil;

/// Something the Almagest set can draw: one of the volume's 27 glyphs,
/// or a project's own derived sigil.
///
/// A sigil is carried as the **project name** rather than as the
/// derived [`Sigil`], and that is the point: the name is the mark's
/// identity, [`Sigil::of`] is a pure function of it, and so a `Mark` is
/// `Copy`, `Eq` and `Hash` — which is what lets the atlas key on the
/// mark itself instead of on a bag of floats that cannot be hashed.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Mark<'a> {
    /// One of the set's glyphs.
    Icon(Icon),
    /// The sigil of the project called this.
    Sigil(&'a str),
}

impl Mark<'_> {
    /// The mark's elements on the 16-unit grid.
    #[must_use]
    pub fn shapes(self) -> Vec<Shape> {
        match self {
            Self::Icon(icon) => icon.shapes().to_vec(),
            Self::Sigil(name) => Sigil::of(name).shapes(),
        }
    }
}

#[cfg(test)]
#[path = "mark.tests.rs"]
mod tests;
