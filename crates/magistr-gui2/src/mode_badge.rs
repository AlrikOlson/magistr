use qs_gpu::color::Srgba;

use crate::component::Component;
use crate::element::Element;
use crate::lexicon::Lexicon;
use crate::voice::Voice;

/// A permission rule's standing answer — the volume's `.mode7`: a small tracked mono
/// badge whose word and hue are one vocabulary (`ASK` warns, `AUTO` is a green standing
/// yes, `OFF` a red standing no). mode badge: the copy is the variant's, never free
/// text, and like every read-only class it can carry no id.
pub struct ModeBadge;

impl ModeBadge {
    /// `.mode7 { padding:1px 8px; border-radius:4px; border:1px solid }`
    /// — the border-box the capture measures includes the 1px ring.
    const PAD_X: f32 = 9.0;
    const PAD_Y: f32 = 2.0;
    const RADIUS: f32 = 4.0;
    /// The ring keeps half its hue
    /// (`color-mix(in oklab, var(--*) 50%, transparent)`).
    const RING_ALPHA: f32 = 0.5;

    /// `ASK` — the rule stops and asks, in the warning hue.
    #[must_use]
    pub fn ask(ground: Srgba, lex: &Lexicon) -> Element {
        Self::badge("ASK", lex.yellow, ground)
    }

    /// `AUTO` — a standing yes, in the success hue.
    #[must_use]
    pub fn auto(ground: Srgba, lex: &Lexicon) -> Element {
        Self::badge("AUTO", lex.green, ground)
    }

    /// `OFF` — a standing no, in the danger hue.
    #[must_use]
    pub fn off(ground: Srgba, lex: &Lexicon) -> Element {
        Self::badge("OFF", lex.red, ground)
    }

    /// One badge on `ground` — the translucent ring is precomposed
    /// there in sRGB bytes, the way the browser blends it.
    fn badge(word: &'static str, hue: Srgba, ground: Srgba) -> Element {
        let ring = Lexicon::composite(ground, Srgba::new(hue.r, hue.g, hue.b, Self::RING_ALPHA));
        Element::row()
            .pad_x(Self::PAD_X)
            .pad_y(Self::PAD_Y)
            .radius(Self::RADIUS)
            .stroke(ring, 1.0)
            .child(
                Element::text(Voice::BADGE, hue, word)
                    .claim(Component::ModeBadge)
                    .nowrap(),
            )
    }
}

#[cfg(test)]
#[path = "mode_badge.tests.rs"]
mod tests;
