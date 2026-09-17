use magistr_cockpit::cockpit::Progress;
use qs_gpu::color::Srgba;

use crate::component::Component;
use crate::element::Element;
use crate::lexicon::Lexicon;
use crate::thin_bar::ThinBar;
use crate::voice::Voice;

/// The strip under a card's head: the program's own progress report, as it wrote it to
/// its terminal (OSC 9;4), drawn as the volume's thin bar with its percent beside it in
/// the cap voice — the success hue while it runs, the failure hue and ink when the
/// program says so, and an empty track reading `working` when it reports no fraction.
/// Nothing at all when the program reported nothing.
pub struct ProgressStrip;

impl ProgressStrip {
    /// The bar's width — the card's data column, not the whole card.
    pub const BAR_WIDTH: f32 = 160.0;
    /// The air between the bar and its label.
    const GAP: f32 = 8.0;

    /// The strip for `progress`, `None` when there is nothing to show.
    #[must_use]
    pub fn el(progress: Progress, lex: &Lexicon) -> Option<Element> {
        let (share, label, hue, ink): (f32, String, Srgba, Srgba) = match progress {
            Progress::Cleared => return None,
            Progress::Running(p) => (Self::share(p), format!("{p}%"), lex.green, lex.sb1),
            Progress::Failed(p) => (Self::share(p), format!("{p}%"), lex.red, lex.red),
            Progress::Indeterminate => (0.0, "working".to_owned(), lex.green, lex.sb1),
        };
        Some(
            Element::row()
                .gap(Self::GAP)
                .child(ThinBar::hued(share, Self::BAR_WIDTH, hue, lex))
                .child(
                    Element::text(Voice::CAP, ink, label)
                        .claim(Component::Cap)
                        .nowrap(),
                ),
        )
    }

    /// A percent as the bar's share.
    fn share(percent: u8) -> f32 {
        f32::from(percent.min(100)) / 100.0
    }
}

#[cfg(test)]
#[path = "progress_strip.tests.rs"]
mod tests;
