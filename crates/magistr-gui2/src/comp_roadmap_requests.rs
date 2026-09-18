//! Bounded request transcripts use shared admission and completion ranges.
use crate::comp_roadmap::RoadmapComposition as R;
use crate::comp_workspace::{control, heading, panel};
use crate::{Element, Lexicon, Voice};
use magistr_cockpit::cockpit::{Model, RequestPhase, Widget};
use magistr_cockpit::turn::{ConversationCard, LineKind, Voice as TurnVoice};

pub(crate) struct RequestsComposition;
impl RequestsComposition {
    pub fn el(model: &Model, height: f32, lex: &Lexicon) -> Element {
        let mut head = heading("Requests", lex).child(Element::row().grow(1.0));
        if !model.roadmap_requests().is_empty() {
            head = head.child(control("Latest reply", Widget::RequestLatest.id(), lex));
        }
        let mut content = Element::surface()
            .gap(12.0)
            .min_width(0.0)
            .no_shrink()
            .id(R::REQUEST_CONTENT);
        if model.roadmap_requests().is_empty() {
            content=content.child(Element::text(Voice::LEDGER_NAME,lex.tx,"What should change?"))
                .child(prose("Ask to add work, change priorities, or revise the plan. Your requests and replies stay here.",lex))
                .child(prose("Updating the roadmap does not start an item.",lex));
        }
        for request in model.roadmap_requests() {
            let mut card = Element::surface()
                .gap(10.0)
                .pad_y(12.0)
                .no_shrink()
                .min_width(0.0)
                .child(Element::text(Voice::LEDGER_NAME, lex.tx, "You"))
                .child(prose(&request.text, lex))
                .child(
                    Element::text(
                        Voice::LEDGER_NAME,
                        lex.mauve,
                        format!("Magistr · {}", request.phase.label()),
                    )
                    .min_width(0.0),
                );
            match request.phase {
                RequestPhase::Queued => {
                    card=card.child(prose("This request will start after the current work finishes. You can cancel it below.",lex));
                }
                RequestPhase::Waiting => {
                    card = card.child(prose("Waiting for the agent to accept this request.", lex));
                }
                _ => {}
            }
            for line in request.response(model.turn()) {
                match line.kind() {
                    LineKind::Prose(TurnVoice::User | TurnVoice::Thought) | LineKind::Event => {}
                    LineKind::Tool { .. } | LineKind::Edit { .. } | LineKind::Check { .. } => {
                        if let Some(tool) = ConversationCard::of(model.turn(), line) {
                            card = card
                                .child(
                                    Element::text(Voice::LEDGER_NAME, lex.tx, tool.title)
                                        .min_width(0.0),
                                )
                                .child(
                                    Element::text(Voice::HINT, lex.sb0, tool.status).min_width(0.0),
                                );
                        }
                    }
                    _ => {
                        card = card.child(prose(line.text(), lex));
                    }
                }
            }
            if let Some(error) = &request.error {
                card = card.child(Element::text(Voice::STEER, lex.red, error).min_width(0.0));
            }
            content = content.child(card);
        }
        panel(height, lex)
            .child(head)
            .child(
                Element::surface()
                    .height(0.0)
                    .grow(1.0)
                    .min_height(0.0)
                    .id(R::REQUESTS)
                    .child(content),
            )
            .claim(crate::Component::Prose)
    }
}
fn prose(text: &str, lex: &Lexicon) -> Element {
    Element::text(Voice::STEER, lex.sb1, text)
        .min_width(0.0)
        .claim(crate::Component::Prose)
}
