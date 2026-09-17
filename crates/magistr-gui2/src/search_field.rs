use crate::component::Component;
use crate::element::Element;
use crate::element_id::ElementId;
use crate::lexicon::Lexicon;
use crate::voice::Voice;

/// Where a person finds rather than jumps — the volume's search
/// field: the chord in the accent, the query in the text ink with its
/// caret, the live count in the faint ink, all inside a hairline at
/// the panel radius. Takes focus; a search is typed into.
pub struct SearchField;

impl SearchField {
    /// `padding:8px 14px` inside the 1px ring, `border-radius:11px`,
    /// `gap:9px`.
    const PAD_X: f32 = 15.0;
    const PAD_Y: f32 = 9.0;
    const RADIUS: f32 = 11.0;
    const GAP: f32 = 9.0;
    /// The chord's voice — the inline `mono 11px`.
    const CHORD: Voice = Voice {
        px: 11.0,
        ..Voice::CHIP
    };
    /// The caret — `.caret7`.
    const CARET_W: f32 = 1.5;
    const CARET_H: f32 = 14.0;

    /// The field mid-search: `chord` (the volume's `⌘⇧F`), the typed
    /// `query` with its caret, and the `count` at the far edge.
    #[must_use]
    pub fn el(chord: &str, query: &str, count: &str, id: ElementId, lex: &Lexicon) -> Element {
        Element::row()
            .pad_x(Self::PAD_X)
            .pad_y(Self::PAD_Y)
            .gap(Self::GAP)
            .center()
            .radius(Self::RADIUS)
            .stroke(lex.grounded(lex.hairc), 1.0)
            .id(id)
            .focusable()
            .focus_ring(lex.mauve)
            .child(
                Element::text(Self::CHORD, lex.mauve, chord)
                    .claim(Component::Field)
                    .nowrap(),
            )
            .child(
                Element::row()
                    .grow(1.0)
                    .center()
                    .child(
                        Element::text(Voice::STEER, lex.tx, query)
                            .claim(Component::Field)
                            .nowrap(),
                    )
                    .child(
                        Element::surface()
                            .width(Self::CARET_W)
                            .height(Self::CARET_H)
                            .fill(lex.mauve),
                    ),
            )
            .child(
                Element::text(Voice::KBD, lex.ov0, count)
                    .claim(Component::Field)
                    .nowrap(),
            )
    }
}

#[cfg(test)]
#[path = "search_field.tests.rs"]
mod tests;
