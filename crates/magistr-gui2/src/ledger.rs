use qs_gpu::color::Srgba;

use crate::component::Component;
use crate::element::Element;
use crate::lexicon::Lexicon;
use crate::voice::Voice;

/// The receipts at rest — the volume's ledger block: a cap over key/value lines in the
/// data voice on a double leading, every value right-aligned in its own ink (a hash in
/// the subdued ink, a gate verdict in green, a cost as a plain numeral). receipt
/// vocabulary in its block form; read-only.
pub struct Ledger;

impl Ledger {
    /// The block's `line-height:2` on the 10.5px data voice — the row
    /// pitch every line sits on.
    const VOICE: Voice = Voice {
        line_height: 2.0,
        ..Voice::CHIP
    };

    /// The ledger: `cap` over one line per `(key, value, ink)` row.
    #[must_use]
    pub fn el(cap: &str, rows: &[(&str, &str, Srgba)], lex: &Lexicon) -> Element {
        let pitch = Self::VOICE.px * Self::VOICE.leading();
        let mut block = Element::surface().child(
            Element::row().height(pitch).center().child(
                Element::text(Voice::CAP, lex.ov0, cap)
                    .claim(Component::Ledger)
                    .nowrap(),
            ),
        );
        for (key, value, ink) in rows {
            block = block.child(
                Element::row()
                    .height(pitch)
                    .center()
                    .space_between()
                    .child(
                        Element::text(Self::VOICE, lex.sb0, *key)
                            .claim(Component::Ledger)
                            .nowrap(),
                    )
                    .child(
                        Element::text(Self::VOICE, *ink, *value)
                            .claim(Component::Ledger)
                            .nowrap(),
                    ),
            );
        }
        block
    }
}

#[cfg(test)]
#[path = "ledger.tests.rs"]
mod tests;
