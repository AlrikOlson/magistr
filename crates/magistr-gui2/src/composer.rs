use crate::component::Component;
use crate::element::Element;
use crate::element_id::ElementId;
use crate::lexicon::Lexicon;
use crate::voice::Voice;

/// Where the person speaks — the volume's composer pill: a stadium
/// hairline around the conditional register's glyph and the resting
/// prompt, the caret standing in the accent. The one control here that
/// takes focus: a composer is written into.
pub struct Composer;

impl Composer {
    /// The pill's `padding:7px 14px` inside its 1px ring, `gap:9px`.
    const PAD_X: f32 = 15.0;
    const PAD_Y: f32 = 8.0;
    const GAP: f32 = 9.0;
    /// The register glyph's voice — the inline `mono 11px`.
    const REGISTER: Voice = Voice {
        px: 11.0,
        ..Voice::CHIP
    };
    /// The caret — `.caret7 { width:1.5px; height:14px }`.
    const CARET_W: f32 = 1.5;
    const CARET_H: f32 = 14.0;
    /// The session pill's measured box (view 05: ring rows 551..588),
    /// which the pill's own padding under-reaches with the embedded
    /// face's line height.
    const SESSION_HEIGHT: f32 = 37.0;

    /// The composer mid-thought: the same pill with `text` — what the
    /// reader has typed — in the full ink, the caret standing after it.
    /// The resting face ([`Composer::el`]) wears the faint prompt
    /// instead; which face shows is the caller's read of the model,
    /// never state here.
    #[must_use]
    pub fn filled(register: &str, text: &str, id: ElementId, lex: &Lexicon) -> Element {
        Self::pill(register, text, id, lex, true)
    }

    /// The composer at rest: `register` (the volume's `›`), the
    /// resting `prompt` in the faint ink, the standing caret.
    #[must_use]
    pub fn el(register: &str, prompt: &str, id: ElementId, lex: &Lexicon) -> Element {
        Self::pill(register, prompt, id, lex, false)
    }

    /// The session pill (view 05): [`Composer::el`]'s resting face with
    /// `hint` — the volume's send/cancel caps — right-aligned inside
    /// the ring. Its own face, not the atom's: the specimen pins the
    /// bare pill, and the hints are the session composition's.
    #[must_use]
    pub fn hinted(
        register: &str,
        prompt: &str,
        hint: &str,
        id: ElementId,
        lex: &Lexicon,
    ) -> Element {
        Self::pill(register, prompt, id, lex, false)
            .height(Self::SESSION_HEIGHT)
            .child(
                Element::text(Voice::CAP, lex.ov0, hint)
                    .claim(Component::Field)
                    .nowrap(),
            )
    }

    /// [`Composer::filled`] with the same right-aligned `hint`.
    #[must_use]
    pub fn hinted_filled(
        register: &str,
        text: &str,
        hint: &str,
        id: ElementId,
        lex: &Lexicon,
    ) -> Element {
        Self::pill(register, text, id, lex, true)
            .height(Self::SESSION_HEIGHT)
            .child(
                Element::text(Voice::CAP, lex.ov0, hint)
                    .claim(Component::Field)
                    .nowrap(),
            )
    }

    fn pill(register: &str, prompt: &str, id: ElementId, lex: &Lexicon, filled: bool) -> Element {
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
                Element::text(Self::REGISTER, lex.mauve, register)
                    .claim(Component::Field)
                    .nowrap(),
            )
            .child(
                Element::row()
                    .grow(1.0)
                    .center()
                    .child(
                        Element::text(Voice::SLOT, if filled { lex.tx } else { lex.ov0 }, prompt)
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
#[path = "composer.tests.rs"]
mod tests;
