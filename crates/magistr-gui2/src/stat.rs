use crate::component::Component;
use crate::element::Element;
use crate::lexicon::Lexicon;
use crate::voice::Voice;

/// A number over its total — the volume's counter: `211 / 306` in the data voice, the
/// current count sitting in its own fixed slot (`.cnt { width:30px }`) so the digits
/// can change without the ` / total` tail breathing. stat: a measurement, read-only.
///
/// The volume crossfades an old and a new count in that slot; the stat's rest face is
/// the standing count alone, which is also what the frozen capture holds and what
/// reduced motion collapses to.
pub struct Stat;

impl Stat {
    /// The counter slot's width — `.cnt { width:30px }`.
    const SLOT: f32 = 30.0;

    /// `count` in its slot, then ` / total` — one row of the stat
    /// voice in the second subdued ink.
    #[must_use]
    pub fn el(count: &str, total: &str, lex: &Lexicon) -> Element {
        Element::row()
            .child(
                Element::row().width(Self::SLOT).child(
                    Element::text(Voice::STAT, lex.sb1, count)
                        .claim(Component::Stat)
                        .nowrap(),
                ),
            )
            .child(
                Element::text(Voice::STAT, lex.sb1, format!(" / {total}"))
                    .claim(Component::Stat)
                    .nowrap(),
            )
    }
}

#[cfg(test)]
#[path = "stat.tests.rs"]
mod tests;
