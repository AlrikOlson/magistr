use crate::component::Component;
use crate::element::Element;
use crate::element_id::ElementId;
use crate::lexicon::Lexicon;
use crate::voice::Voice;

/// A line the reader writes their own words on — the volume's own-answer
/// field on the interview card and the ground card's typed path: a
/// stadium hairline, the accent's `›` prompt mark, the text with its
/// caret, and the placeholder in the faint ink while nothing is typed.
/// Takes focus; an answer is typed into.
pub struct AnswerField;

impl AnswerField {
    /// `padding:7px 14px` inside the 1px ring, `gap:10px`,
    /// `border-radius:999px`.
    const PAD_X: f32 = 15.0;
    const PAD_Y: f32 = 8.0;
    const GAP: f32 = 10.0;
    /// The prompt mark's voice — the inline `mono 11px`.
    const MARK: Voice = Voice {
        px: 11.0,
        ..Voice::CHIP
    };
    /// The text's voice — the inline sans 12.
    const TEXT: Voice = Voice::SLOT;
    /// The caret — `.caret7`.
    const CARET_W: f32 = 1.5;
    const CARET_H: f32 = 14.0;

    /// The field with `text` typed so far (the `placeholder` shows in
    /// its place while it is empty), answering focus on `id`.
    #[must_use]
    pub fn el(text: &str, placeholder: &str, id: ElementId, lex: &Lexicon) -> Element {
        let (copy, ink) = if text.is_empty() {
            (placeholder, lex.ov0)
        } else {
            (text, lex.tx)
        };
        Element::row()
            .pad_x(Self::PAD_X)
            .pad_y(Self::PAD_Y)
            .gap(Self::GAP)
            .center()
            .radius(999.0)
            .stroke(lex.grounded(lex.hairc), 1.0)
            .id(id)
            .focusable()
            .focus_ring(lex.mauve)
            .child(
                Element::text(Self::MARK, lex.mauve, "\u{203a}")
                    .claim(Component::Field)
                    .nowrap(),
            )
            .child(
                Element::row()
                    .grow(1.0)
                    .center()
                    .child(
                        Element::text(Self::TEXT, ink, copy)
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
    }
}

#[cfg(test)]
#[path = "answer_field.tests.rs"]
mod tests;
