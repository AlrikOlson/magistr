use crate::component::Component;
use crate::element::Element;
use crate::element_id::ElementId;
use crate::lexicon::Lexicon;
use crate::voice::Voice;

/// navigation class: a flat chip naming a place — `.chip7` with a mauve waypoint dot in
/// the volume. It raises on hover and dives on click, and unlike a
/// [`DataPill`](crate::pill::DataPill) it *is* interactive: it wears an id, takes
/// focus, and answers the pointer.
pub struct NavChip;

impl NavChip {
    /// One chip naming `place`. Rest face is the data chip's — flat, hairline ring —
    /// because the design separates the classes by *behavior* (a nav chip answers) and
    /// by the dot, not by a louder skin.
    #[must_use]
    pub fn el(place: &str, id: ElementId, lex: &Lexicon) -> Element {
        Element::row()
            .pad_x(12.0)
            .pad_y(4.0)
            .gap(6.0)
            .center()
            .radius(999.0)
            .stroke(lex.grounded(lex.hairc), 1.0)
            // The raise and the dive: the pointer lightens the ground a
            // step, the press drops it below rest.
            .hover_fill(lex.mantle)
            .press_fill(lex.deep)
            .id(id)
            .focusable()
            .focus_ring(lex.mauve)
            .child(
                Element::surface()
                    .width(6.0)
                    .height(6.0)
                    .radius(3.0)
                    .fill(lex.mauve),
            )
            .child(
                Element::text(Voice::CHIP, lex.sb0, place)
                    .claim(Component::Chip)
                    .nowrap(),
            )
    }
}

#[cfg(test)]
#[path = "nav_chip.tests.rs"]
mod tests;
