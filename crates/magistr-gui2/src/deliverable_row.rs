use qs_gpu::color::Srgba;

use crate::card::Card;
use crate::component::Component;
use crate::element::Element;
use crate::icon::Icon;
use crate::lexicon::Lexicon;
use crate::mark::Mark;
use crate::voice::Voice;

/// One deliverable that is not a picture, on the Report's shelf (view 09): the session
/// recording, the motion filmstrips — a `.float7` row with a 26-px badge wearing the
/// kind's glyph in its hue (a disc for the recording, a rounded square for the
/// filmstrips), the kind's name in the UI voice and its measure in the small mono
/// beside it. a "Session recording · 12 min" line is this component, never naked prose.
pub struct DeliverableRow;

impl DeliverableRow {
    /// `.float7 { padding:9px 12px }` on the row, `gap:10px`.
    const PAD_X: f32 = 12.0;
    const PAD_Y: f32 = 9.0;
    const GAP: f32 = 10.0;
    /// The badge (`width:26px; height:26px`) and its glyph at the set's
    /// own 13 px; a disc or the 6-px square.
    const BADGE: f32 = 26.0;
    const BADGE_ICON_PX: u32 = 13;
    const SQUARE_RADIUS: f32 = 6.0;
    /// The air between the name and its measure — the volume's one
    /// space.
    const META_GAP: f32 = 4.0;
    /// The name's voice — the volume's inline sans 12.
    const NAME: Voice = Voice {
        family: "sans",
        px: 12.0,
        weight: 400,
        tracking_em: 0.0,
        line_height: 0.0,
        upper: false,
    };
    /// The measure's voice — the volume's inline mono 9.5.
    const META: Voice = Voice {
        family: "mono",
        px: 9.5,
        weight: 400,
        tracking_em: 0.0,
        line_height: 0.0,
        upper: false,
    };

    /// One row: `icon` in `hue` on a `round` disc or a rounded square,
    /// `name` and its `meta` (`12 min`, `3 strips`), on the mantle at a
    /// row's lift.
    #[must_use]
    pub fn el(
        icon: Icon,
        hue: Srgba,
        round: bool,
        name: &str,
        meta: &str,
        lex: &Lexicon,
    ) -> Element {
        let badge = Element::row()
            .width(Self::BADGE)
            .height(Self::BADGE)
            .radius(if round {
                Self::BADGE / 2.0
            } else {
                Self::SQUARE_RADIUS
            })
            .stroke(Lexicon::composite(lex.mantle, lex.hairc), 1.0)
            .center()
            .justify_center()
            .child(Element::mark(
                Mark::Icon(icon),
                Self::BADGE_ICON_PX,
                Lexicon::ink_bytes(hue),
            ));
        let words = Element::row()
            .gap(Self::META_GAP)
            .center()
            .grow(1.0)
            .child(
                Element::text(Self::NAME, lex.tx, name)
                    .claim(Component::DeliverableRow)
                    .nowrap(),
            )
            .child(
                Element::text(Self::META, lex.ov0, format!("\u{b7} {meta}"))
                    .claim(Component::DeliverableRow)
                    .nowrap(),
            );
        Card::el(Self::PAD_X, Self::PAD_Y, lex).child(
            Element::row()
                .gap(Self::GAP)
                .center()
                .child(badge)
                .child(words),
        )
    }
}

#[cfg(test)]
#[path = "deliverable_row.tests.rs"]
mod tests;
