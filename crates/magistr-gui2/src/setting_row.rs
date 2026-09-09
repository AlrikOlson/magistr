use qs_gpu::color::Srgba;

use crate::component::Component;
use crate::element::Element;
use crate::lexicon::Lexicon;
use crate::voice::Voice;

/// One setting on a pane (view 10's Interface rows): the setting's name over its mono
/// note, the control at the far edge, on a hairline when another row follows. The
/// control is whatever the content asks for — a switch, a segmented select, a value —
/// and the row only seats it.
pub struct SettingRow;

impl SettingRow {
    /// `gap:16px; padding:11px 0`.
    const GAP: f32 = 16.0;
    const PAD_Y: f32 = 11.0;
    /// The name's voice — the inline sans 12.5/550.
    const NAME: Voice = Voice {
        weight: 550,
        ..Voice::STEER
    };

    /// The row: `name` and its `note`, `control` at the edge, divided
    /// from the next by the hairline when `divided`.
    #[must_use]
    pub fn el(
        name: &str,
        note: &str,
        control: Element,
        divided: bool,
        ground: Srgba,
        lex: &Lexicon,
    ) -> Element {
        let row = Element::row()
            .pad_y(Self::PAD_Y)
            .gap(Self::GAP)
            .center()
            .child(
                Element::surface()
                    .grow(1.0)
                    .min_width(0.0)
                    .child(
                        Element::text(Self::NAME, lex.tx, name)
                            .claim(Component::SettingRow)
                            .nowrap(),
                    )
                    .child(
                        Element::text(Voice::LEDGER_META, lex.ov0, note)
                            .claim(Component::SettingRow)
                            .nowrap(),
                    ),
            )
            .child(control);
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
#[path = "setting_row.tests.rs"]
mod tests;
