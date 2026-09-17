use qs_gpu::color::Srgba;

use crate::component::Component;
use crate::element::Element;
use crate::element_id::ElementId;
use crate::lexicon::Lexicon;
use crate::voice::Voice;

/// Selection, for a search's scope: the volume's `.chip7` worn as a single
/// select — the picked chip on the accent glow's 14% wash in the text ink with its ring
/// gone, the others flat on the hairline ring in the subtle ink. A choice, never an
/// action: picking one narrows the same query.
///
/// Not [`Segments`](crate::segments::Segments): those share one stadium; these stand
/// apart with air between them, which is how the volume draws a scope.
pub struct ScopeChip;

impl ScopeChip {
    /// `padding:3px 11px` inside a 1px border — the border-box the
    /// chip keeps whether or not its ring is drawn.
    const PAD_X: f32 = 12.0;
    const PAD_Y: f32 = 4.0;
    /// The picked wash — `color-mix(in oklab, var(--glow-m) 14%, transparent)`.
    const WASH: f32 = 0.14;

    /// One chip reading `label`, `picked` or not, on `ground`,
    /// answering on `id`.
    #[must_use]
    pub fn el(label: &str, picked: bool, ground: Srgba, id: ElementId, lex: &Lexicon) -> Element {
        let mut chip = Element::row()
            .pad_x(Self::PAD_X)
            .pad_y(Self::PAD_Y)
            .center()
            .radius(999.0)
            .id(id)
            .focusable()
            .focus_ring(lex.mauve);
        let ink = if picked {
            let wash = Srgba::new(lex.glow_m.r, lex.glow_m.g, lex.glow_m.b, Self::WASH);
            chip = chip.fill(Lexicon::composite(ground, wash));
            lex.tx
        } else {
            chip = chip
                .stroke(Lexicon::composite(ground, lex.hairc), 1.0)
                .hover_fill(lex.mantle)
                .press_fill(lex.deep);
            lex.sb0
        };
        chip.child(
            Element::text(Voice::CHIP, ink, label)
                .claim(Component::Chip)
                .nowrap(),
        )
    }
}

#[cfg(test)]
#[path = "scope_chip.tests.rs"]
mod tests;
