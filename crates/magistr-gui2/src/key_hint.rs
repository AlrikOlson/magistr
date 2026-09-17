use qs_gpu::color::Srgba;

use crate::component::Component;
use crate::element::Element;
use crate::lexicon::Lexicon;
use crate::medium::Medium;
use crate::voice::Voice;

/// A key and what it does — the volume's `.khint7`: a kbd cap beside a plain verb, in
/// the subdued ink. key-hint pair: the one legal way to say "press this"; a bare `⏎` in
/// copy is a missing component. Read-only like every hint.
pub struct KeyHint;

impl KeyHint {
    /// `.khint7 { gap:6px }`.
    const GAP: f32 = 6.0;
    /// The cap's border-box paddings — `.kbd7 { padding:1px 6px;
    /// border:1px }` measured through the border the box includes.
    const CAP_PAD_X: f32 = 7.0;
    const CAP_PAD_Y: f32 = 2.0;
    const CAP_RADIUS: f32 = 5.0;
    /// `.kbd7 { opacity:.85 }` — folded into every layer's alpha at
    /// precompose time, because the substrate paints groups opaque.
    const DIM: f32 = 0.85;

    /// One pair: `cap` on its key face, `label` beside it, on `ground`.
    #[must_use]
    pub fn el(cap: &str, label: &str, medium: Medium, ground: Srgba, lex: &Lexicon) -> Element {
        Self::in_voice(cap, label, Voice::HINT, medium, ground, lex)
    }

    /// The pair with its label in `voice` — the ledger head sets its
    /// pairs a size down from the class ([`Voice::LEDGER_HINT`]); the
    /// cap keeps the kbd's own voice, which the volume never varies.
    #[must_use]
    pub fn in_voice(
        cap: &str,
        label: &str,
        voice: Voice,
        medium: Medium,
        ground: Srgba,
        lex: &Lexicon,
    ) -> Element {
        Element::row()
            .gap(Self::GAP)
            .center()
            .child(Self::cap(cap, medium, ground, lex))
            .child(
                Element::text(voice, lex.sb0, label)
                    .claim(Component::KeyHint)
                    .nowrap(),
            )
    }

    /// The cap alone — the volume's standalone `.kbd7` face (the
    /// gradient buttons carry their own darker variant inside
    /// `action.rs`): a well in the night, a faint lilac wash in the
    /// day, each precomposed on `ground` with the cap's own dimming.
    fn cap(copy: &str, medium: Medium, ground: Srgba, lex: &Lexicon) -> Element {
        let over = |tint: Srgba, alpha: f32| {
            Lexicon::composite(
                ground,
                Srgba::new(tint.r, tint.g, tint.b, alpha * Self::DIM),
            )
        };
        // The volume's own literals: night wears rgba(0,0,0,.28) under
        // a rgba(255,255,255,.14) ring; day rgba(64,60,90,.08) under
        // rgba(64,60,90,.18).
        let lilac = Srgba::new(64.0 / 255.0, 60.0 / 255.0, 90.0 / 255.0, 1.0);
        let (bed, ring) = match medium {
            Medium::Night => {
                let bed = over(Srgba::new(0.0, 0.0, 0.0, 1.0), 0.28);
                (
                    bed,
                    Lexicon::composite(bed, Srgba::new(1.0, 1.0, 1.0, 0.14 * Self::DIM)),
                )
            }
            Medium::Day => (over(lilac, 0.08), over(lilac, 0.18)),
        };
        let ink = Lexicon::composite(bed, Srgba::new(lex.sb0.r, lex.sb0.g, lex.sb0.b, Self::DIM));
        Element::row()
            .pad_x(Self::CAP_PAD_X)
            .pad_y(Self::CAP_PAD_Y)
            .radius(Self::CAP_RADIUS)
            .fill(bed)
            .stroke(ring, 1.0)
            .child(
                Element::text(Voice::KBD, ink, copy)
                    .claim(Component::KeyHint)
                    .nowrap(),
            )
    }
}

#[cfg(test)]
#[path = "key_hint.tests.rs"]
mod tests;
