use magistr_cockpit::cockpit::{Copy, Model};

use crate::component::Component;
use crate::element::Element;
use crate::element_id::ElementId;
use crate::lexicon::Lexicon;
use crate::medium::Medium;
use crate::nav_chip::NavChip;
use crate::pill::DataPill;
use crate::voice::Voice;

/// The identity band — the session's cap, one row: the chunk being worked as a
/// navigation chip that rises to its star on the Sky · the agent block's `model · mode`
/// · the iteration `4 / 12` · the spend `0.61 USD` · the stage word with its live dot.
/// The clock stays in the header's chip and the tokens with their headroom in the foot.
///
/// A composition: every run is a component and every unit goes through [`Copy`]; it
/// maps the model and draws nothing of its own. Each slot arrives with its news — no
/// chip for a chunk the rail does not name, no agent chip before the handshake named a
/// model, no spend before the agent priced anything, no counter without a loop — so a
/// fresh session's band is quiet rather than furniture.
pub struct BandComposition;

impl BandComposition {
    /// The chunk chip — the band's one control.
    pub const CHUNK: ElementId = ElementId(520);
    /// The air between the band's chips (the volume's `gap:8px`).
    const GAP: f32 = 8.0;
    /// The chunk title's room on the chip, in characters: a roadmap title is a claim
    /// and can run to a sentence, and the chip is a name. Past this the title is cut at
    /// a word with the ellipsis — the composition's own cut until the text run can
    /// ellipsize itself.
    pub const TITLE_CHARS: usize = 40;

    /// The band for `model`, in `medium`'s lexicon.
    #[must_use]
    pub fn el(model: &Model, medium: Medium) -> Element {
        let lex = Lexicon::of(medium);
        let mut row = Element::row().gap(Self::GAP).center();
        if let Some(id) = model.rail().chunk() {
            row = row.child(NavChip::el(
                &Self::chunk_label(model, id),
                Self::CHUNK,
                &lex,
            ));
        }
        if let Some(agent) = model.rail().agent() {
            row = row.child(DataPill::label(&agent.identity(), &lex));
        }
        if let Some(status) = model.loop_status() {
            row = row.child(
                Element::text(
                    Voice::CHIP,
                    lex.sb1,
                    format!("{} / {}", status.iteration(), status.max()),
                )
                .claim(Component::Counter)
                .nowrap(),
            );
        }
        if let Some(cents) = model.rail().spent_cents() {
            row = row.child(DataPill::num(&Copy::usd(cents), &lex));
        }
        if let Some(ask) = model.ask() {
            // A parked loop is never silent: the wait is named in the strip's own
            // words, in the alarm's hue — the one chip on the band that asks for you.
            row = row.child(DataPill::alarm(
                &format!("needs you: {}", ask.title()),
                lex.peach,
                &lex,
            ));
        } else if let Some(word) = Self::stage_word(model) {
            row = row.child(DataPill::marked(&word, lex.mauve, &lex));
        }
        row
    }

    /// The chunk ID and, when available, its title truncated to [`Self::TITLE_CHARS`].
    /// Unknown chunks display only their ID.
    fn chunk_label(model: &Model, id: &str) -> String {
        let title = model
            .roadmap()
            .chunk(id)
            .map(|chunk| chunk.title.as_str())
            .filter(|title| !title.is_empty() && *title != id);
        match title {
            Some(title) => format!("{id} \u{b7} {}", Self::clip(title)),
            None => id.to_owned(),
        }
    }

    /// `title` cut to [`Self::TITLE_CHARS`] at the last word boundary
    /// before the cut, with the ellipsis; a title that fits is itself.
    fn clip(title: &str) -> String {
        if title.chars().count() <= Self::TITLE_CHARS {
            return title.to_owned();
        }
        let head: String = title.chars().take(Self::TITLE_CHARS).collect();
        let cut = head.rfind(' ').filter(|at| *at > 0).unwrap_or(head.len());
        format!("{}\u{2026}", head[..cut].trim_end())
    }

    /// The stage word: where the loop stands — `loop ended` once it
    /// stopped, `paused` while it waits, else the driver's own word
    /// (`agent working`, `gates running`, `judge deciding`) — and,
    /// with no loop, `agent working` while the agent is doing something
    /// the cockpit cannot see the inside of. `None` when nothing moves.
    fn stage_word(model: &Model) -> Option<String> {
        match model.loop_status() {
            Some(status) if status.ended().is_some() => Some("loop ended".to_owned()),
            Some(status) if status.is_paused() => Some("paused".to_owned()),
            Some(status) => Some(status.stage().label().to_owned()),
            None if model.is_working() => Some("agent working".to_owned()),
            None => None,
        }
    }
}

#[cfg(test)]
#[path = "comp_band.tests.rs"]
mod tests;
