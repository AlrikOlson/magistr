use qs_gpu::color::Srgba;

use crate::component::Component;
use crate::element::Element;
use crate::element_id::ElementId;
use crate::lexicon::Lexicon;
use crate::voice::Voice;

/// Selection, for an answer offered as a chip: the volume's `.chip7` worn as
/// a pick — picked, the ok face (`.chip7.ok`: green ink, the green ring, a check after
/// the word); unpicked, flat on the hairline ring in the subdued ink. A choice, never
/// an action: picking one adds it to the answer.
///
/// Not [`ScopeChip`](crate::scope_chip::ScopeChip): a scope is one of several, a pick
/// is any of them.
pub struct PickChip;

impl PickChip {
    /// `padding:3px 11px` inside a 1px border.
    const PAD_X: f32 = 12.0;
    const PAD_Y: f32 = 4.0;
    /// The picked ring keeps 45% of the green glow
    /// (`color-mix(in oklab, var(--glow-g) 45%, transparent)`).
    const RING_ALPHA: f32 = 0.45;

    /// One chip reading `label`, `picked` or not, on `ground`,
    /// answering on `id`.
    #[must_use]
    pub fn el(label: &str, picked: bool, ground: Srgba, id: ElementId, lex: &Lexicon) -> Element {
        let chip = Element::row()
            .pad_x(Self::PAD_X)
            .pad_y(Self::PAD_Y)
            .center()
            .radius(999.0)
            .id(id)
            .focusable()
            .focus_ring(lex.mauve)
            .hover_fill(lex.mantle)
            .press_fill(lex.deep);
        if picked {
            let ring = Srgba::new(lex.glow_g.r, lex.glow_g.g, lex.glow_g.b, Self::RING_ALPHA);
            chip.stroke(Lexicon::composite(ground, ring), 1.0).child(
                Element::text(Voice::CHIP, lex.green, format!("{label} \u{2713}"))
                    .claim(Component::Chip)
                    .nowrap(),
            )
        } else {
            chip.stroke(Lexicon::composite(ground, lex.hairc), 1.0)
                .child(
                    Element::text(Voice::CHIP, lex.sb0, label)
                        .claim(Component::Chip)
                        .nowrap(),
                )
        }
    }
}

#[cfg(test)]
#[path = "pick_chip.tests.rs"]
mod tests;
