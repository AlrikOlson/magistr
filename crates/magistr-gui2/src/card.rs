use magistr_cockpit::theme::Severity;

use crate::element::Element;
use crate::elevation::Elevation;
use crate::lexicon::Lexicon;
use crate::severity::SeverityInk;

/// A floating surface — the volume's `.float7`: the lifted neutral ground at the card
/// radius, standing off the canvas by elevation (surface shift + the two-layer shadow),
/// **never** by a border. The three grounds are the volume's own three: the mantle, a
/// severity's 9% tint over it, and the overlay glass (`.ov7` — the mantle at 84% over
/// what it covers).
///
/// The shift half of elevation lives in the color itself — `mantle` *is* the base
/// lifted — and the depth half in the [`Elevation`] the card carries, which the emitter
/// renders as the two shadows and the rim. A 1px perimeter stroke or a coloured left
/// strip is not expressible here, which is the design prohibition held by type.
pub struct Card;

impl Card {
    /// `.float7 { border-radius:13px }`. Crate visible: the breathe
    /// halo wraps a card at its own corner.
    pub(crate) const RADIUS: f32 = 13.0;
    /// `.ov7 { background:color-mix(in oklab, var(--mantle) 84%, transparent) }`.
    const GLASS: f32 = 0.84;

    /// The plain card: the mantle at a row's lift.
    #[must_use]
    pub fn el(pad_x: f32, pad_y: f32, lex: &Lexicon) -> Element {
        Self::ground(lex.mantle, Elevation::row(), pad_x, pad_y)
    }

    /// A severity card: the band's tinted ground, same lift — the ground half of
    /// treatment (the cap half is [`SeverityCap`](crate::SeverityCap)).
    #[must_use]
    pub fn severity(severity: Severity, pad_x: f32, pad_y: f32, lex: &Lexicon) -> Element {
        Self::ground(
            SeverityInk::of(severity, lex).ground,
            Elevation::row(),
            pad_x,
            pad_y,
        )
    }

    /// A card on a hue that is not a severity band, same recipe and
    /// same lift — the blue a researched signal's card wears (view 12).
    #[must_use]
    pub fn tinted(hue: qs_gpu::color::Srgba, pad_x: f32, pad_y: f32, lex: &Lexicon) -> Element {
        Self::ground(Self::tint(hue, lex), Elevation::row(), pad_x, pad_y)
    }

    /// The tinted ground itself — `hue` at the volume's 9% over the
    /// mantle — for a caller that goes on to mix it further (a settled
    /// card at its opacity).
    #[must_use]
    pub fn tint(hue: qs_gpu::color::Srgba, lex: &Lexicon) -> qs_gpu::color::Srgba {
        SeverityInk::tinted(hue, lex).ground
    }

    /// The overlay: glass over the canvas at the overlay's lift.
    ///
    /// The glass is precomposed over `behind` — the browser blurs what
    /// the overlay covers and lets 16% of it through; a still built on
    /// a flat ground passes that ground.
    #[must_use]
    pub fn overlay(behind: qs_gpu::color::Srgba, pad_x: f32, pad_y: f32, lex: &Lexicon) -> Element {
        Self::ground(Self::glass(behind, lex), Elevation::overlay(), pad_x, pad_y)
    }

    /// The popover: the same glass at the top of the scale.
    #[must_use]
    pub fn popover(behind: qs_gpu::color::Srgba, pad_x: f32, pad_y: f32, lex: &Lexicon) -> Element {
        Self::ground(Self::glass(behind, lex), Elevation::popover(), pad_x, pad_y)
    }

    /// The overlay's own ground over `behind` — what a row *inside* an
    /// overlay card composites its hairline onto, so a receipt row's
    /// rule on the Sky's card is the rule the capture shows.
    #[must_use]
    pub fn overlay_ground(behind: qs_gpu::color::Srgba, lex: &Lexicon) -> qs_gpu::color::Srgba {
        Self::glass(behind, lex)
    }

    /// The glass ground over `behind`.
    fn glass(behind: qs_gpu::color::Srgba, lex: &Lexicon) -> qs_gpu::color::Srgba {
        Lexicon::composite(
            behind,
            qs_gpu::color::Srgba::new(lex.mantle.r, lex.mantle.g, lex.mantle.b, Self::GLASS),
        )
    }

    fn ground(fill: qs_gpu::color::Srgba, elevation: Elevation, pad_x: f32, pad_y: f32) -> Element {
        Element::surface()
            .pad_x(pad_x)
            .pad_y(pad_y)
            .radius(Self::RADIUS)
            .fill(fill)
            .elevation(elevation)
    }
}

#[cfg(test)]
#[path = "card.tests.rs"]
mod tests;
