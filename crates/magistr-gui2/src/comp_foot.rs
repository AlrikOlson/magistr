use magistr_cockpit::cockpit::{Copy, Model, Widget};

use crate::component::Component;
use crate::element::Element;
use crate::lexicon::Lexicon;
use crate::medium::Medium;
use crate::phase_rail::PhaseRail;
use crate::voice::Voice;

/// The window's foot, composed from the model: the volume's bottom bar — the
/// iteration's phase rail on the left and the running token total on the right. A
/// decision screen (a Moment up) carries the phase the decision is being made *in* and
/// nothing else — no run of phases, no totals — and both slots arrive with their news:
/// a session with no phase yet shows the four words quiet, one that never reported
/// usage shows no count.
pub struct FootComposition;

impl FootComposition {
    /// The bar's band — the volume's own `FOOTER_HEIGHT`. Crate
    /// visible: the Galaxy's foot is the same band, and the root
    /// measures the field against it.
    pub(crate) const HEIGHT: f32 = 34.75;
    /// The bar's side margins (`FOOTER_PAD_SIDE`).
    const PAD_SIDE: f32 = 20.0;

    /// The foot for `model`, in `medium`'s lexicon.
    #[must_use]
    pub fn el(model: &Model, medium: Medium) -> Element {
        let lex = Lexicon::of(medium);
        let deciding = model.moment().is_some();
        let current = model
            .phase()
            .and_then(Widget::phase)
            .and_then(|lit| Widget::PHASES.iter().position(|phase| *phase == lit));
        let labels: Vec<String> = Widget::PHASES
            .iter()
            .map(|phase| phase.label().to_ascii_lowercase())
            .collect();
        let words: Vec<&str> = labels.iter().map(String::as_str).collect();
        let mut bar = Element::row()
            .height(Self::HEIGHT - 1.75)
            .pad_x(Self::PAD_SIDE)
            .center();
        bar = bar.child(match (deciding, current) {
            // The decision's own phase, alone.
            (true, Some(at)) => PhaseRail::el(&words[at..=at], 0, &lex),
            (true, None) => Element::row(),
            // The run of phases, the live one lit.
            (false, Some(at)) => PhaseRail::el(&words, at, &lex),
            // No phase yet: every word ahead — the rail cannot say
            // "none live", so the quiet face is spelled here in its
            // own ahead ink.
            (false, None) => {
                let mut quiet = Element::row().gap(12.0).center();
                for word in &words {
                    quiet = quiet.child(
                        Element::text(Voice::PHASE, lex.ov0, *word)
                            .claim(Component::PhaseRail)
                            .nowrap(),
                    );
                }
                quiet
            }
        });
        bar = bar.child(Element::row().grow(1.0));
        // The level, and beside it what the window has left — both halves of the one
        // usage update, so neither shows before the agent reported at all.
        if let Some(level) = model.token_level().filter(|_| !deciding) {
            let mut copy = format!("{} tok", thousands(level));
            if let Some(left) = model.token_headroom() {
                copy.push_str(&format!(" \u{b7} {} left", Copy::kilo(left)));
            }
            bar = bar.child(
                Element::text(Voice::PHASE, lex.sb0, copy)
                    .claim(Component::Counter)
                    .nowrap(),
            );
        }
        // The bar opens on its own hairline (the volume's border-top,
        // inside `FOOTER_HEIGHT` the way border-box counts it). The
        // 0.75 shim keeps the line on the capture's own pixel row: the
        // band starts at y 605.25 and the capture inks 606..607.
        Element::surface()
            .child(Element::surface().height(0.75))
            .child(Element::surface().height(1.0).fill(lex.grounded(lex.hairc)))
            .child(bar)
    }
}

/// `38204` as `38,204` — grouped the way the capture prints it.
fn thousands(n: u64) -> String {
    let digits = n.to_string();
    let mut out = String::with_capacity(digits.len() + digits.len() / 3);
    for (index, ch) in digits.chars().enumerate() {
        if index > 0 && (digits.len() - index).is_multiple_of(3) {
            out.push(',');
        }
        out.push(ch);
    }
    out
}

#[cfg(test)]
#[path = "comp_foot.tests.rs"]
mod tests;
