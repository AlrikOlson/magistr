//! Selected record text from the cockpit; native wrapping and scrolling stay here.
use crate::comp_workspace::{WorkspaceComposition as W, control, heading, panel};
use crate::{Element, Lexicon, Voice};
use magistr_cockpit::cockpit::{
    Model, Selected, Widget, inspection::Inspection, inspector::Inspector,
};

pub(crate) struct DetailsComposition;

impl DetailsComposition {
    pub(crate) fn el(model: &Model, height: f32, lex: &Lexicon) -> Element {
        let head = heading("Details", lex)
            .child(Element::row().grow(1.0))
            .child(control(
                if model.inspector_open() {
                    "Less"
                } else {
                    "More"
                },
                Widget::InspectorDetail.id(),
                lex,
            ));
        let mut body = Element::surface()
            .gap(8.0)
            .min_width(0.0)
            .no_shrink()
            .id(W::DETAIL_CONTENT);
        let card = Inspection::of(model);
        if let Some(card) = card {
            body = body
                .child(Element::text(Voice::LEDGER_NAME, lex.tx, &card.title).min_width(0.0))
                .child(Element::text(Voice::HINT, lex.mauve, &card.status).min_width(0.0))
                .child(Element::text(Voice::HINT, lex.sb0, &card.metadata).min_width(0.0));
            if let Some(Selected::Step(number)) = model.selected() {
                body = body.child(
                    if let Some((name, _)) = Inspector::linked_work(model, *number) {
                        control(&format!("Work: {name}"), Widget::LinkedWork.id(), lex)
                    } else {
                        Element::text(
                            Voice::HINT,
                            lex.sb0,
                            if model
                                .trace()
                                .step(*number)
                                .is_some_and(|s| s.execution_ref.is_some())
                            {
                                "Linked work is no longer available."
                            } else {
                                "No linked work"
                            },
                        )
                    },
                );
            }
            for (label, value) in card
                .fields
                .iter()
                .filter(|(_, value)| !value.trim().is_empty())
            {
                body = body
                    .child(Element::text(Voice::LEDGER_NAME, lex.tx, label))
                    .child(Element::text(Voice::STEER, lex.sb1, value).min_width(0.0));
            }
            if let Some(diff) = &card.diff {
                body = body.child(Element::text(Voice::DIFFLINE, lex.sb1, diff).min_width(0.0));
            }
            if let Some(lines) = &card.viewer {
                body = body
                    .child(Element::text(Voice::STEER, lex.sb1, lines.join("\n")).min_width(0.0));
            }
            for (label, target) in card.actions {
                body = body.child(control(label, target, lex));
            }
            for (index, proposal) in card.proposals {
                body = body
                    .child(Element::text(Voice::STEER, lex.sb1, proposal))
                    .child(
                        Element::row()
                            .child(control("Accept", Widget::proposal_accept(index), lex))
                            .child(control("Reject", Widget::proposal_reject(index), lex)),
                    );
            }
        } else {
            body = body.child(Element::text(
                Voice::STEER,
                lex.sb0,
                if model.selected().is_some() {
                    "This record is no longer available."
                } else {
                    "Select a step or work item to see its details."
                },
            ));
        }
        panel(height, lex)
            .child(head)
            .child(
                Element::surface()
                    .grow(1.0)
                    .height(0.0)
                    .min_height(0.0)
                    .id(W::DETAILS)
                    .child(body),
            )
            .claim(crate::Component::Prose)
    }
}
