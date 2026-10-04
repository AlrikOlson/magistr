//! Native request text uses shared documents; native layout supplies its source positions.
use crate::comp_roadmap::RoadmapComposition as R;
use crate::comp_workspace::{WorkspaceComposition as W, control, heading, panel};
use crate::rich_text::RichText;
use crate::{Element, Lexicon, Voice};
use magistr_cockpit::cockpit::{Model, ReadingPosition, RequestHistory, Widget};

pub(crate) struct RequestsComposition;
impl RequestsComposition {
    pub fn el(model: &Model, height: f32, lex: &Lexicon) -> Element {
        let head = heading("Requests", lex)
            .child(Element::row().grow(1.0))
            .child(control("Previous section", Widget::request_jump(0), lex))
            .child(control("Next section", Widget::request_jump(1), lex))
            .child(control("Copy", Widget::request_copy(), lex))
            .child(control("Latest reply", Widget::RequestLatest.id(), lex));
        let mut content = Element::surface()
            .gap(8.0)
            .min_width(0.0)
            .no_shrink()
            .id(R::REQUEST_CONTENT);
        let history = RequestHistory::of(model);
        if history.sections.is_empty() {
            content = content.child(Element::text(Voice::LEDGER_NAME, lex.tx, "What should change?"))
                .child(Element::text(Voice::STEER, lex.sb1, "Ask to add work, change priorities, or revise the plan. Your requests and replies stay here.").min_width(0.0));
        }
        let mut at = 0;
        for section in &history.sections {
            if !section.heading.is_empty() {
                content = content.child(
                    Element::text(Voice::LEDGER_NAME, lex.tx, &section.heading)
                        .min_width(0.0)
                        .id(W::id(Widget::request_block(at))),
                );
                at += 1;
            }
            for block in section.text.blocks() {
                let root = at;
                at += 1;
                let mut body = Element::surface().min_width(0.0).no_shrink();
                if block.cells.is_some() {
                    for (label, value) in block.cell_rows() {
                        body = body.child(RichText::cell(label, value, lex));
                    }
                } else {
                    let text = RichText::marked(&block.row, lex.tx, lex, &mut |_| {
                        let id = W::id(Widget::request_block(at));
                        at += 1;
                        Some(id)
                    });
                    body = if let Some(marker) = &block.marker {
                        Element::row()
                            .min_width(0.0)
                            .child(Element::text(Voice::STEER, lex.sb1, marker).no_shrink())
                            .child(text.grow(1.0).min_width(0.0))
                    } else {
                        body.child(text)
                    };
                }
                if block.row.text.is_empty() && block.cells.is_none() {
                    body = body.height(8.0);
                }
                content = content.child(body.id(W::id(Widget::request_block(root))));
            }
        }
        for section in &history.sections {
            if let Some(target) = section.action {
                content = content.child(
                    control(&format!("{} · Open details", section.heading), target, lex)
                        .min_width(0.0),
                );
            }
        }
        for (index, (label, address)) in history.links.iter().enumerate() {
            let focused = RequestHistory::is_focused_link(model, Widget::request_link(index));
            content = content.child(
                control(
                    &format!(
                        "{}{label} · {address} · Copy link",
                        if focused { "› " } else { "" }
                    ),
                    Widget::request_link(index),
                    lex,
                )
                .min_width(0.0),
            );
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

    pub(crate) fn positions(model: &Model) -> Vec<ReadingPosition> {
        let history = RequestHistory::of(model);
        let mut positions = Vec::new();
        for section in &history.sections {
            if !section.heading.is_empty() {
                positions.push(ReadingPosition {
                    section: section.key.clone(),
                    source: None,
                });
            }
            for block in section.text.blocks() {
                positions.push(ReadingPosition {
                    section: section.key.clone(),
                    source: Some(block.anchor),
                });
                if block.cells.is_none() {
                    let mut byte = 0;
                    for word in block.row.text.split_inclusive(' ') {
                        positions.push(ReadingPosition {
                            section: section.key.clone(),
                            source: Some(block.source_position(byte)),
                        });
                        byte += word.len();
                    }
                }
            }
        }
        positions
    }
}
