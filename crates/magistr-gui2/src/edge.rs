use crate::element::Element;
use crate::lexicon::Lexicon;

/// A dependency between two stars — the constellation's edge: a 1.4px
/// line in the edge ink, drawn solid behind work already `done` and
/// dashed `4 4` ahead of it. Read-only; the sky owns the endpoints.
pub struct Edge;

impl Edge {
    /// The volume's `stroke-width:1.4`.
    const WIDTH: f32 = 1.4;
    /// The ahead edge's `stroke-dasharray="4 4"`.
    const DASH: f32 = 4.0;

    /// A settled dependency: one solid line of `length`.
    #[must_use]
    pub fn done(length: f32, lex: &Lexicon) -> Element {
        Element::row()
            .width(length)
            .height(Self::WIDTH)
            .fill(lex.edge)
    }

    /// A dependency still ahead: the same line broken `4 4`.
    #[must_use]
    pub fn ahead(length: f32, lex: &Lexicon) -> Element {
        let mut line = Element::row()
            .width(length)
            .height(Self::WIDTH)
            .gap(Self::DASH);
        let mut laid = 0.0;
        while laid < length {
            let dash = Self::DASH.min(length - laid);
            line = line.child(
                Element::surface()
                    .width(dash)
                    .height(Self::WIDTH)
                    .fill(lex.edge),
            );
            laid += Self::DASH * 2.0;
        }
        line
    }
}

#[cfg(test)]
#[path = "edge.tests.rs"]
mod tests;
