use crate::card::Card;
use crate::component::Component;
use crate::element::Element;
use crate::icon::Icon;
use crate::lexicon::Lexicon;
use crate::mark::Mark;
use crate::voice::Voice;

/// One fact the survey found (Birth F2's Detected column) — a `.float7` row at
/// `padding:9px 13px` carrying its kind as a teal mono key (the kind's glyph and its
/// word) and the fact in the UI voice beside it, wrapping under the key's baseline. a
/// "Rust · edition 2024" line is this component, never naked prose.
pub struct FindingRow;

impl FindingRow {
    /// `padding:9px 13px`, `gap:9px`.
    const PAD_X: f32 = 13.0;
    const PAD_Y: f32 = 9.0;
    const GAP: f32 = 9.0;
    /// The key's glyph is the set's own 13px beside a 10px mono word
    /// (`gap:4px`).
    const KEY_ICON_PX: u32 = 13;
    const KEY_GAP: f32 = 4.0;
    /// The key's voice — the inline `mono 10px`.
    const KEY: Voice = Voice {
        px: 10.0,
        ..Voice::CHIP
    };
    /// The fact's voice — the inline sans 12.5.
    const FACT: Voice = Voice {
        px: 12.5,
        ..Voice::SLOT
    };

    /// The row: `icon` and `key` in the teal, `fact` beside them,
    /// `width` pixels wide.
    #[must_use]
    pub fn el(icon: Icon, key: &str, fact: &str, width: f32, lex: &Lexicon) -> Element {
        let inner = width - 2.0 * Self::PAD_X;
        Card::el(Self::PAD_X, Self::PAD_Y, lex)
            .width(width)
            .align_start()
            .child(
                Element::row()
                    .gap(Self::GAP)
                    .align_start()
                    .width(inner)
                    .child(
                        Element::row()
                            .no_shrink()
                            .gap(Self::KEY_GAP)
                            .center()
                            .child(Element::mark(
                                Mark::Icon(icon),
                                Self::KEY_ICON_PX,
                                Lexicon::ink_bytes(lex.teal),
                            ))
                            .child(
                                Element::text(Self::KEY, lex.teal, key)
                                    .claim(Component::FindingRow)
                                    .nowrap(),
                            ),
                    )
                    .child(
                        Element::text(Self::FACT, lex.tx, fact)
                            .claim(Component::FindingRow)
                            .min_width(0.0),
                    ),
            )
    }
}

#[cfg(test)]
#[path = "finding_row.tests.rs"]
mod tests;
