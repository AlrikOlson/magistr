use crate::component::Component;
use crate::element::Element;
use crate::lexicon::Lexicon;
use crate::voice::Voice;

/// The iteration's four words and where it stands — the volume's
/// phases indicator: each finished phase in the success hue with its
/// check, the live one in the accent at weight 600, everything ahead
/// in the faint ink. A reading of the loop, so like every indicator
/// it can carry no id.
pub struct PhaseRail;

impl PhaseRail {
    /// The indicator's `gap:12px`.
    const GAP: f32 = 12.0;
    /// The check a finished phase wears, after the volume's own space.
    const DONE: &'static str = " \u{2713}";

    /// The rail: `phases` in order, everything before `current` done,
    /// `current` live, the rest ahead.
    #[must_use]
    pub fn el(phases: &[&str], current: usize, lex: &Lexicon) -> Element {
        let live = Voice {
            weight: 600,
            ..Voice::PHASE
        };
        let mut rail = Element::row().gap(Self::GAP).center();
        for (at, phase) in phases.iter().enumerate() {
            rail = rail.child(if at < current {
                Element::text(Voice::PHASE, lex.green, format!("{phase}{}", Self::DONE))
                    .claim(Component::PhaseRail)
                    .nowrap()
            } else if at == current {
                Element::text(live, lex.mauve, *phase)
                    .claim(Component::PhaseRail)
                    .nowrap()
            } else {
                Element::text(Voice::PHASE, lex.ov0, *phase)
                    .claim(Component::PhaseRail)
                    .nowrap()
            });
        }
        rail
    }
}

#[cfg(test)]
#[path = "phase_rail.tests.rs"]
mod tests;
