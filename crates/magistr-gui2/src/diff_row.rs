use qs_gpu::color::Srgba;

use crate::component::Component;
use crate::element::Element;
use crate::lexicon::Lexicon;
use crate::voice::Voice;

/// One line of a diff — the volume's `.diffline`: the code voice on
/// its own 1.75 leading, an added line on a faint success wash, a
/// deleted one on a faint danger wash in the subdued ink, context in
/// the faint ink on no wash at all. Read-only, like everything a
/// receipt shows.
pub struct DiffRow;

impl DiffRow {
    /// `.diffline { padding:0 12px }`.
    const PAD_X: f32 = 12.0;
    /// The washes' shares — `color-mix(in oklab, var(--glow-g) 12%, …)`
    /// and `…var(--red) 11%…`.
    const ADD_WASH: f32 = 0.12;
    const DEL_WASH: f32 = 0.11;

    /// An added line — `+ …` on the success wash.
    #[must_use]
    pub fn add(code: &str, ground: Srgba, lex: &Lexicon) -> Element {
        let wash = Srgba::new(lex.glow_g.r, lex.glow_g.g, lex.glow_g.b, Self::ADD_WASH);
        Self::row(code, lex.tx, Some(Lexicon::composite(ground, wash)))
    }

    /// A deleted line — `- …` on the danger wash, in the subdued ink.
    #[must_use]
    pub fn del(code: &str, ground: Srgba, lex: &Lexicon) -> Element {
        let wash = Srgba::new(lex.red.r, lex.red.g, lex.red.b, Self::DEL_WASH);
        Self::row(code, lex.sb0, Some(Lexicon::composite(ground, wash)))
    }

    /// A context line — no wash, the faint ink.
    #[must_use]
    pub fn ctx(code: &str, lex: &Lexicon) -> Element {
        Self::row(code, lex.ov0, None)
    }

    fn row(code: &str, ink: Srgba, wash: Option<Srgba>) -> Element {
        let mut row = Element::row().pad_x(Self::PAD_X);
        if let Some(wash) = wash {
            row = row.fill(wash);
        }
        row.child(
            Element::text(Voice::DIFFLINE, ink, code)
                .claim(Component::Diff)
                .nowrap(),
        )
    }
}

#[cfg(test)]
#[path = "diff_row.tests.rs"]
mod tests;
