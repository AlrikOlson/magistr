use magistr_cockpit::cockpit::{Binding, LoopStatus, Model, Widget};
use qs_gpu::color::Srgba;

use crate::element::Element;
use crate::element_id::ElementId;
use crate::key_hint::KeyHint;
use crate::ledger_verbs::LedgerVerbs;
use crate::lexicon::Lexicon;
use crate::medium::Medium;
use crate::voice::Voice;

/// The work a window can start or steer, decided once for every place that draws it:
/// while no loop is live and the session's host starts work in place, the two starts —
/// Start next item · Start loop; while a loop is live, the loop's own verbs; otherwise
/// nothing. The rail's ledger head and the conversation heading both read this, so the
/// two cannot offer different verbs for one model.
///
/// Every verb is the cockpit's own widget: a press, Return on the focused control and
/// an accessibility click all take the fold the keys take.
pub struct WorkControls;

impl WorkControls {
    /// The start-next pair on the rail — after the ledger verbs' four.
    pub const NEXT: ElementId = ElementId(504);
    /// The start-loop pair on the rail.
    pub const LOOP: ElementId = ElementId(505);
    /// `gap:8px` between the pairs, the ledger verbs' own.
    const GAP: f32 = 8.0;

    /// The verbs `model` offers now, in drawing order: the widget, what it is called
    /// and the binding whose keys it wears.
    #[must_use]
    pub fn verbs(model: &Model) -> Vec<(Widget, &'static str, Binding)> {
        if model.loop_is_live() {
            let paused = model.loop_status().is_some_and(LoopStatus::is_paused);
            return vec![
                (
                    Widget::LoopPause,
                    if paused { "Resume" } else { "Pause" },
                    Binding::PAUSE,
                ),
                (Widget::LoopStep, "Step", Binding::STEP),
                (
                    Widget::LoopCheckpoint,
                    "Checkpoint",
                    Binding::CHECKPOINT_NOW,
                ),
                (Widget::LoopAbort, "Abort", Binding::ABORT_CHORD),
            ];
        }
        if model.hosts_work() {
            return vec![
                (Widget::LaunchNext, "Start next item", Binding::NEXT),
                (Widget::LaunchLoop, "Start loop", Binding::LOOP),
            ];
        }
        Vec::new()
    }

    /// The rail's row for `model` on `ground`: the ledger head's verbs while a loop is
    /// live, the starts as the same key-hint pairs while idle, `None` with nothing to
    /// offer.
    #[must_use]
    pub fn rail(model: &Model, medium: Medium, ground: Srgba, lex: &Lexicon) -> Option<Element> {
        if model.loop_is_live() {
            let paused = model.loop_status().is_some_and(LoopStatus::is_paused);
            return Some(LedgerVerbs::el(paused, medium, ground, lex));
        }
        let verbs = Self::verbs(model);
        if verbs.is_empty() {
            return None;
        }
        let mut row = Element::row().wrap().gap(Self::GAP).center();
        for (widget, label, binding) in verbs {
            let id = if widget == Widget::LaunchNext {
                Self::NEXT
            } else {
                Self::LOOP
            };
            row = row.child(
                KeyHint::in_voice(binding.keys, label, Voice::LEDGER_HINT, medium, ground, lex)
                    .id(id)
                    .focusable()
                    .focus_ring(lex.mauve)
                    .label(label),
            );
        }
        Some(row)
    }
}

#[cfg(test)]
#[path = "work_controls.tests.rs"]
mod tests;
