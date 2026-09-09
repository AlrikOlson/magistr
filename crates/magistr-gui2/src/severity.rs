use magistr_cockpit::theme::Severity;
use qs_gpu::color::Srgba;

use crate::lexicon::Lexicon;
use crate::oklab;

/// What one [`Severity`] band looks like on the specimen sheet: the tinted ground the
/// whole card takes and the full-strength cap ink — pair, and *only* that pair, so a
/// strip stays inexpressible here exactly as it is on the band itself.
///
/// The band is the cockpit's own; this type is where it meets the volume's palette. The
/// ground is the volume's own recipe — `color-mix(in oklab, <state> 9%,
/// var(--mantle))`, the mix every severity card in the seventh edition wears — and the
/// cap is the state code at full strength. [`Severity::Quiet`] is the one band that is
/// not a signal: its cap is the faint ink and its ground is the mantle untinted.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SeverityInk {
    /// The card's whole ground, tinted at low saturation.
    pub ground: Srgba,
    /// The cap's ink — the hue at full strength.
    pub cap: Srgba,
}

impl SeverityInk {
    /// How much of the state code the tinted ground takes — the `9%`
    /// in the volume's own `color-mix`.
    const TINT: f32 = 0.09;

    /// The band's pair under `lex`'s medium.
    #[must_use]
    pub fn of(severity: Severity, lex: &Lexicon) -> Self {
        let signal = match severity {
            Severity::Alert => lex.red,
            Severity::Attention => lex.peach,
            Severity::Good => lex.green,
            Severity::Quiet => {
                return Self {
                    ground: lex.mantle,
                    cap: lex.ov0,
                };
            }
        };
        Self::tinted(signal, lex)
    }

    /// The same pair for a hue that is not a severity band — the blue a
    /// researched signal's card wears (view 12). The recipe is the one
    /// recipe; the band is one way to name a hue.
    #[must_use]
    pub fn tinted(hue: Srgba, lex: &Lexicon) -> Self {
        Self {
            ground: oklab::mix(hue, lex.mantle, Self::TINT),
            cap: hue,
        }
    }
}

#[cfg(test)]
#[path = "severity.tests.rs"]
mod tests;
