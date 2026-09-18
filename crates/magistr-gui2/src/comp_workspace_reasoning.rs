//! Bounded native reasoning history, selected through the shared trace identities.
use crate::comp_workspace::{WorkspaceComposition as W, control, heading, panel};
use crate::{Element, Lexicon, Voice};
use magistr_cockpit::cockpit::{Model, Selected, Widget};

pub(crate) struct ReasoningComposition;

impl ReasoningComposition {
    pub(crate) fn el(model: &Model, height: f32, lex: &Lexicon) -> Element {
        let open = model.reasoning_expanded();
        let steps = model.trace().steps();
        let title = format!("Reasoning · {}", steps.len());
        let head = heading(&title, lex)
            .child(Element::row().grow(1.0))
            .child(control(
                if open { "Hide" } else { "Show" },
                Widget::ReasoningToggle.id(),
                lex,
            ));
        let mut panel = panel(height, lex).id(W::REASONING).child(head);
        if !open {
            return panel;
        }
        let controls = Element::row()
            .height(30.0)
            .no_shrink()
            .child(control("Previous", Widget::TracePrevious.id(), lex))
            .child(control("Next", Widget::TraceNext.id(), lex))
            .child(Element::row().grow(1.0))
            .child(control("Latest", Widget::LatestDecision.id(), lex));
        panel = panel.child(controls);
        if steps.is_empty() {
            return panel.child(
                Element::text(Voice::STEER, lex.sb0, "No reasoning recorded yet.")
                    .claim(crate::Component::Prose),
            );
        }
        let selected = match model.selected() {
            Some(Selected::Step(n)) => Some(*n),
            _ => None,
        };
        let at = selected
            .and_then(|n| steps.iter().position(|s| s.number == n))
            .unwrap_or(steps.len() - 1);
        let count = (((height - 68.0).max(0.0) / 38.0) as usize).max(1);
        let start = at
            .saturating_sub(count / 2)
            .min(steps.len().saturating_sub(count));
        for (index, step) in steps.iter().enumerate().skip(start).take(count) {
            let label = format!(
                "{} · {}",
                step.number,
                if step.purpose.is_empty() {
                    "Recorded step"
                } else {
                    &step.purpose
                }
            );
            let row = Element::surface()
                .height(38.0)
                .no_shrink()
                .justify_center()
                .child(Element::text(Voice::LEDGER_NAME, lex.tx, &label).nowrap())
                .child(
                    Element::text(
                        Voice::HINT,
                        lex.sb0,
                        step.branch_name.as_deref().unwrap_or("Main investigation"),
                    )
                    .nowrap(),
                )
                .id(W::id(Widget::trace_row(index)))
                .focusable()
                .focus_ring(lex.mauve)
                .label(label);
            panel = panel.child(if selected == Some(step.number) {
                row.fill(lex.s0)
            } else {
                row
            });
        }
        panel.claim(crate::Component::ListRow)
    }
}
