use magistr_cockpit::cockpit::Binding;
use qs_gpu::color::Srgba;

use crate::element::Element;
use crate::element_id::ElementId;
use crate::key_hint::KeyHint;
use crate::lexicon::Lexicon;
use crate::medium::Medium;
use crate::voice::Voice;

/// The ledger head's verbs — `space` Pause · `s` Step · `c` Checkpoint now · `⌘.`
/// Abort, as view 05's rail draws them under `N receipts after cp-N`: a row of key-hint
/// pairs at the rail's 9.5px, 8px apart. Each pair is a control: it wears an element id
/// the root's seam resolves to the cockpit's own loop widget, so a click is the verb
/// its key is. The caps are the registry's keys, never a literal, so a rebound key
/// cannot leave the rail lying. The pause pair reads Resume while the loop holds a
/// boundary.
///
/// Abort wears the window's chord, never the terminal's `esc`: on a window Esc climbs,
/// so the pair is the one abort a pointer has and the chord is the one the keys have.
pub struct LedgerVerbs;

impl LedgerVerbs {
    /// The pause pair — Resume while paused. The four sit past the
    /// palette's row space (400..432).
    pub const PAUSE: ElementId = ElementId(500);
    /// The step pair.
    pub const STEP: ElementId = ElementId(501);
    /// The checkpoint pair.
    pub const CHECKPOINT: ElementId = ElementId(502);
    /// The abort pair.
    pub const ABORT: ElementId = ElementId(503);
    /// The air over the row — `margin-top:5px` under the receipts line.
    pub const TOP: f32 = 5.0;
    /// `gap:8px` between the pairs.
    const GAP: f32 = 8.0;

    /// The row for a loop that is `paused` or not, in `medium`'s
    /// lexicon, its caps precomposed on `ground`.
    #[must_use]
    pub fn el(paused: bool, medium: Medium, ground: Srgba, lex: &Lexicon) -> Element {
        let pause = if paused { "Resume" } else { "Pause" };
        // `flex-wrap:wrap`: a pair that does not fit the spine's column
        // breaks onto the next line rather than widening the column.
        Element::row()
            .wrap()
            .gap(Self::GAP)
            .center()
            .child(pair(
                Binding::PAUSE,
                pause,
                Self::PAUSE,
                medium,
                ground,
                lex,
            ))
            .child(pair(Binding::STEP, "Step", Self::STEP, medium, ground, lex))
            .child(pair(
                Binding::CHECKPOINT_NOW,
                "Checkpoint",
                Self::CHECKPOINT,
                medium,
                ground,
                lex,
            ))
            .child(pair(
                Binding::ABORT_CHORD,
                "Abort",
                Self::ABORT,
                medium,
                ground,
                lex,
            ))
    }
}

/// One pair wearing `binding`'s keys as its cap and `id` as its
/// identity.
fn pair(
    binding: Binding,
    label: &str,
    id: ElementId,
    medium: Medium,
    ground: Srgba,
    lex: &Lexicon,
) -> Element {
    KeyHint::in_voice(binding.keys, label, Voice::LEDGER_HINT, medium, ground, lex).id(id)
}

#[cfg(test)]
#[path = "ledger_verbs.tests.rs"]
mod tests;
