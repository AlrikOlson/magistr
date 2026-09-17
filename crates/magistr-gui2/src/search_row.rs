use qs_gpu::color::Srgba;

use crate::component::Component;
use crate::element::Element;
use crate::element_id::ElementId;
use crate::lexicon::Lexicon;
use crate::voice::Voice;

/// One ⌘⇧F result — the volume's search row (view 13): a chapter's title over its
/// snippet, or a step's, a receipt's, a session's line alone; the chosen row on the
/// accent glow's 12% wash at the row-plus radius, an idle one on nothing at all. result
/// row: a hit is a component with a kind and a date, never a naked line of text.
///
/// The shell is the component; the line — a [`MatchLine`] in the kind's own voice — is
/// the composition's, handed in as an element.
///
/// [`MatchLine`]: crate::match_line::MatchLine
pub struct SearchRow;

impl SearchRow {
    /// `padding:7px 10px; border-radius:9px`.
    const PAD_X: f32 = 10.0;
    const PAD_Y: f32 = 7.0;
    const RADIUS: f32 = 9.0;
    /// The chosen wash — `color-mix(in oklab, var(--glow-m) 12%, transparent)`.
    const WASH: f32 = 0.12;
    /// The title's voice: the volume's sans 12.5, set 550 on the
    /// chosen row (the text ink) and regular on an idle one (the
    /// subtle ink) — the row that is chosen is the one that reads.
    const TITLE: Voice = Voice {
        px: 12.5,
        ..Voice::STEER
    };

    /// The row: `title` (when the kind has one) over `line`, washed
    /// when `chosen`, on `ground`, answering on `id`. `pad_bottom` is
    /// the row's own bottom padding — the volume gives the last row of
    /// the list three more pixels than the rest.
    #[must_use]
    pub fn el(
        title: Option<&str>,
        line: Element,
        chosen: bool,
        pad_bottom: f32,
        ground: Srgba,
        id: ElementId,
        lex: &Lexicon,
    ) -> Element {
        let mut row = Element::surface()
            .pad_x(Self::PAD_X)
            .pad_y(Self::PAD_Y)
            .pad_bottom(pad_bottom)
            .radius(Self::RADIUS)
            .id(id)
            .focusable()
            .focus_ring(lex.mauve);
        if chosen {
            let wash = Srgba::new(lex.glow_m.r, lex.glow_m.g, lex.glow_m.b, Self::WASH);
            row = row.fill(Lexicon::composite(ground, wash));
        }
        if let Some(title) = title {
            let (voice, ink) = if chosen {
                (
                    Voice {
                        weight: 550,
                        ..Self::TITLE
                    },
                    lex.tx,
                )
            } else {
                (Self::TITLE, lex.sb1)
            };
            row = row.child(
                Element::text(voice, ink, title)
                    .claim(Component::SearchRow)
                    .nowrap(),
            );
        }
        row.child(line)
    }
}

#[cfg(test)]
#[path = "search_row.tests.rs"]
mod tests;
