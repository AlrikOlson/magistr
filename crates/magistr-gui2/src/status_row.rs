use qs_gpu::color::Srgba;

use crate::component::Component;
use crate::element::Element;
use crate::lexicon::Lexicon;
use crate::voice::Voice;

/// A status row combines a state dot, object name, state chip, and optional metadata
/// over an optional hairline.
pub struct StatusRow;

impl StatusRow {
    /// `.srow { gap:10px; padding:6px 10px }`.
    const GAP: f32 = 10.0;
    const PAD_X: f32 = 10.0;
    const PAD_Y: f32 = 6.0;
    /// The state dot's diameter (`.dot7`).
    const DOT: f32 = 6.0;
    /// The state chip's border-box paddings — the row's `.chip7`
    /// override (`font-size:9.5px; padding:1px 8px` plus its 1px ring).
    const CHIP_PAD_X: f32 = 9.0;
    const CHIP_PAD_Y: f32 = 2.0;
    /// The state chip's ring keeps 45% of its hue when tinted.
    const CHIP_RING_ALPHA: f32 = 0.45;

    /// One row on `ground`: `dot` colours the state, `name` names the
    /// thing, `state` is the chip's word beside the tint it rides when
    /// the state is a signal, `meta` sits at the far edge, and
    /// `divided` draws the hairline every row but the last wears.
    #[must_use]
    pub fn el(
        dot: Srgba,
        name: &str,
        state: (&str, Option<Srgba>),
        meta: &str,
        divided: bool,
        ground: Srgba,
        lex: &Lexicon,
    ) -> Element {
        let (state, tint) = state;
        let (chip_ink, ring_tint) = match tint {
            Some(hue) => (hue, Srgba::new(hue.r, hue.g, hue.b, Self::CHIP_RING_ALPHA)),
            None => (lex.sb0, lex.hairc),
        };
        let chip_voice = Voice {
            px: 9.5,
            ..Voice::CHIP
        };
        let row = Element::row()
            .pad_x(Self::PAD_X)
            .pad_y(Self::PAD_Y)
            .gap(Self::GAP)
            .center()
            .child(
                Element::surface()
                    .width(Self::DOT)
                    .height(Self::DOT)
                    .radius(Self::DOT / 2.0)
                    .fill(dot),
            )
            .child(
                Element::text(Voice::LEDGER_NAME, lex.sb1, name)
                    .claim(Component::StatusRow)
                    .nowrap(),
            )
            .child(
                Element::row()
                    .pad_x(Self::CHIP_PAD_X)
                    .pad_y(Self::CHIP_PAD_Y)
                    .radius(999.0)
                    .stroke(Lexicon::composite(ground, ring_tint), 1.0)
                    .child(
                        Element::text(chip_voice, chip_ink, state)
                            .claim(Component::StatusRow)
                            .nowrap(),
                    ),
            )
            .child(Element::row().grow(1.0))
            .child(
                Element::text(Voice::LEDGER_META, lex.ov0, meta)
                    .claim(Component::StatusRow)
                    .nowrap(),
            );
        if !divided {
            return row;
        }
        Element::surface().child(row).child(
            Element::surface()
                .height(1.0)
                .fill(Lexicon::composite(ground, lex.hairc)),
        )
    }
}

#[cfg(test)]
#[path = "status_row.tests.rs"]
mod tests;
