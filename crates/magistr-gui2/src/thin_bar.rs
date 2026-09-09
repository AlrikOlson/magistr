use qs_gpu::color::Srgba;

use crate::element::Element;
use crate::lexicon::Lexicon;

/// The 3px budget bar — the volume's thinnest reading: a rounded
/// track on the first surface step with the spent share filled in the
/// success hue. The meter with its labels is
/// [`BudgetMeter`](crate::BudgetMeter); this is the bare bar a footer
/// or a card corner wears. Read-only.
pub struct ThinBar;

impl ThinBar {
    /// `height:3px; border-radius:2px`.
    const HEIGHT: f32 = 3.0;
    const RADIUS: f32 = 2.0;

    /// The bar at `width`, filled to `share`.
    #[must_use]
    pub fn el(share: f32, width: f32, lex: &Lexicon) -> Element {
        Self::hued(share, width, lex.green, lex)
    }

    /// The bar at `width`, filled to `share` in `hue` — the failure
    /// hue for a program reporting an error, the success hue otherwise.
    #[must_use]
    pub fn hued(share: f32, width: f32, hue: Srgba, lex: &Lexicon) -> Element {
        Element::row()
            .width(width)
            .height(Self::HEIGHT)
            .radius(Self::RADIUS)
            .fill(lex.s0)
            .child(
                Element::surface()
                    .width(width * share.clamp(0.0, 1.0))
                    .radius(Self::RADIUS)
                    .fill(hue)
                    .pin_left(0.0)
                    .pin_top(0.0)
                    .pin_bottom(0.0),
            )
    }
}

#[cfg(test)]
#[path = "thin_bar.tests.rs"]
mod tests;
