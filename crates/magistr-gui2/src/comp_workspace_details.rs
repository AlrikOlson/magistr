//! Every selected record uses the shared document; native geometry stays here.
use crate::comp_workspace::{WorkspaceComposition as W, control, heading, panel};
use crate::rich_text::RichText;
use crate::{Element, Lexicon, Voice};
use magistr_cockpit::cockpit::{Inspection, Model, ReadingPosition, RecordTarget, Widget};

pub(crate) struct DetailsComposition;
impl DetailsComposition {
    pub(crate) fn el(model: &Model, height: f32, lex: &Lexicon) -> Element {
        let head = heading("Details", lex)
            .child(Element::row().grow(1.0))
            .child(control("Copy", Widget::detail_copy(), lex));
        let mut header = Element::surface().gap(4.0).no_shrink().child(head);
        let mut body = Element::surface()
            .gap(8.0)
            .min_width(0.0)
            .no_shrink()
            .id(W::DETAIL_CONTENT);
        if let Some(card) = Inspection::of(model) {
            header = header
                .child(
                    Element::text(Voice::LEDGER_NAME, lex.tx, &card.title)
                        .min_width(0.0)
                        .id(W::id(Widget::InspectorTitle.id())),
                )
                .child(
                    Element::text(Voice::HINT, lex.mauve, &card.status)
                        .min_width(0.0)
                        .id(W::id(Widget::InspectorStatus.id())),
                )
                .child(
                    Element::text(Voice::HINT, lex.sb0, &card.metadata)
                        .min_width(0.0)
                        .id(W::id(Widget::InspectorMetadata.id())),
                );
            let mut actions = Element::row().wrap().gap(8.0).no_shrink();
            for (label, target) in &card.actions {
                actions = actions.child(control(label, *target, lex));
            }
            if card.viewer {
                actions = actions.child(control("Close", Widget::InspectorDetail.id(), lex));
            }
            header = header.child(actions);
            let mut at = 0;
            for (index, section) in card.sections.iter().enumerate() {
                let open = card.is_expanded(model, section);
                let label = format!(
                    "{}{}",
                    section.heading,
                    if section.secondary {
                        if open { " · Hide" } else { " · Show" }
                    } else {
                        ""
                    }
                );
                let title = if section.secondary {
                    control(&label, Widget::detail_section(index), lex)
                } else {
                    Element::text(Voice::LEDGER_NAME, lex.tx, label).min_width(0.0)
                };
                body = body.child(
                    Element::surface()
                        .id(W::id(Widget::detail_block(at)))
                        .child(title),
                );
                at += 1;
                if !open {
                    continue;
                }
                for block in section.text.blocks() {
                    let root = at;
                    at += 1;
                    let mut content = Element::surface().min_width(0.0).no_shrink();
                    if block.cells.is_some() {
                        for (label, value) in block.cell_rows() {
                            content = content.child(RichText::cell(label, value, lex));
                        }
                    } else {
                        let mut text = RichText::marked(&block.row, lex.tx, lex, &mut |_| {
                            let id = W::id(Widget::detail_block(at));
                            at += 1;
                            Some(id)
                        });
                        if block.heading > 0 {
                            text = text.pad_y(if block.heading == 1 { 6.0 } else { 2.0 });
                        }
                        content = if let Some(marker) = &block.marker {
                            Element::row()
                                .min_width(0.0)
                                .child(Element::text(Voice::STEER, lex.sb1, marker).no_shrink())
                                .child(text.grow(1.0).min_width(0.0))
                        } else {
                            content.child(text)
                        };
                    }
                    if block.row.text.is_empty() && block.cells.is_none() {
                        content = content.height(8.0);
                    }
                    body = body.child(content.id(W::id(Widget::detail_block(root))));
                }
                for link in &section.links {
                    let label = match &link.target {
                        RecordTarget::Record(_) => format!("{} →", link.label),
                        RecordTarget::Address(address) => {
                            format!("{} · {address} · Copy link", link.label)
                        }
                        RecordTarget::Unavailable => "This record is no longer available.".into(),
                    };
                    body = body.child(if link.target == RecordTarget::Unavailable {
                        Element::text(Voice::HINT, lex.sb0, label).min_width(0.0)
                    } else {
                        control(&label, link.widget, lex).min_width(0.0)
                    });
                }
                for (label, target) in &section.actions {
                    body = body.child(control(label, *target, lex));
                }
            }
        } else {
            body = body.child(Element::text(
                Voice::STEER,
                lex.sb0,
                if model.selected().is_some() {
                    "This record is no longer available."
                } else {
                    "Select a record to see details."
                },
            ));
        }
        panel(height, lex)
            .child(header)
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

    pub(crate) fn positions(model: &Model) -> Vec<ReadingPosition> {
        let Some(card) = Inspection::of(model) else {
            return Vec::new();
        };
        let mut positions = Vec::new();
        for section in &card.sections {
            positions.push(ReadingPosition {
                section: section.key.clone(),
                source: None,
            });
            if card.is_expanded(model, section) {
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
        }
        positions
    }
}
