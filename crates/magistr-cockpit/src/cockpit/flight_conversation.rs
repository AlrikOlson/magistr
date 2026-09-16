//! The live, source-anchored conversation in the Flight Recorder workspace.
use super::{FlightStyle as S, Model, Selected, Widget};
use crate::layout::{Length, Node};
use crate::pixel::Typography;
use crate::theme::Pen;
use crate::turn::{ConversationCard, ConversationRow, LineKind, RowKind, Voice};

/// Speaker groups and recorded work share the stream's source rows and real actions.
pub struct FlightConversation;

#[cfg(test)]
#[path = "flight_conversation.tests.rs"]
mod tests;

impl FlightConversation {
    #[must_use]
    pub fn view(model: &Model, pen: &Pen) -> Node {
        let s = S::for_model(model);
        let geometry = model.turn().conversation();
        if geometry.height() == 0 {
            return s.prose("Send a message to begin. Your recorded work is available in Records.");
        }
        let layout = model.flight_layout();
        let top = model.top_row();
        let offset = model.conversation_top() - geometry.offset(top) as f32;
        let bottom = model.conversation_top() + model.conversation_height() as f32;
        Node::column()
            .offset_y(-offset)
            .shrink(0.0)
            .min_height(Length::Px(0.0))
            .children(
                geometry
                    .rows()
                    .iter()
                    .skip(top)
                    .filter(|row| row.height > 0)
                    .take_while(|row| (row.top as f32) < bottom)
                    .map(|row| Self::row(model, pen, &s, &layout, row, row.source - top)),
            )
    }

    fn row(
        model: &Model,
        pen: &Pen,
        s: &S,
        layout: &super::FlightLayout,
        row: &ConversationRow,
        visible: usize,
    ) -> Node {
        let compact = layout.conversation_height < 380.0;
        let source = &model.turn().rows()[row.source];
        let line = &model.turn().lines()[row.line];
        let mut node = Node::column()
            .height(Length::Px(row.height as f32))
            .shrink(0.0);
        if let Some(voice) = row.author
            && !(compact && voice == Voice::User)
        {
            let (name, color) = match voice {
                Voice::User => ("You", s.muted),
                Voice::Agent => ("Magistr", s.accent),
                Voice::Thought => ("Magistr · Thinking", s.muted),
            };
            node = node
                .child(S::vspace(
                    (row.before as f32 - s.line_height - 8.0).max(0.0),
                ))
                .child(s.label(name, color).height(Length::Px(s.line_height)))
                .child(S::vspace(8.0));
        }
        let body_height = row.height - row.before - row.after;
        let content = if row.card_head {
            Self::card(model, s, layout, line, body_height)
        } else {
            let mut content = match source.kind {
                RowKind::EditButtons => Self::edit_actions(s, line, visible),
                RowKind::Image => super::view::image_row(model, row.source, pen),
                RowKind::Pane => super::view::pane_row(model, row.source, pen),
                _ => super::view::stage_row(source, model, pen, None),
            };
            let is_card = matches!(
                line.kind(),
                LineKind::Tool { .. } | LineKind::Edit { .. } | LineKind::Check { .. }
            );
            let background = if is_card || matches!(source.kind, RowKind::Code | RowKind::Fence) {
                s.raised
            } else {
                s.panel
            };
            let normal = pen.row(source.kind).fg;
            content.restyle_all(&|style| {
                let mut next = style.clone().bg(background);
                if source.kind == RowKind::Note {
                    next.attrs = next.attrs.without(crate::render::Attrs::ITALIC);
                }
                if next.fg == normal
                    && matches!(
                        source.kind,
                        RowKind::Prose(Voice::Agent | Voice::User) | RowKind::Heading
                    )
                {
                    next = next.fg(s.ink);
                    if source.kind == RowKind::Prose(Voice::User) {
                        next.attrs = next.attrs.without(crate::render::Attrs::BOLD);
                    }
                }
                next
            });
            if matches!(source.kind, RowKind::Code | RowKind::Fence) {
                content = s
                    .ground(content, background)
                    .style(s.style(s.ink).bg(background));
            }
            if source.indent_px > 0 {
                let marker = source.marker.as_deref().unwrap_or("");
                let marker = if marker.trim() == pen.icons.bullet {
                    Node::leaf().mark(crate::layout::Mark::Icon {
                        icon: crate::pixel::Icon::Bullet,
                        col: 0,
                        cols: 1,
                        ink: s.ink,
                    })
                } else {
                    s.label(marker, s.ink)
                };
                content = Node::row()
                    .child(
                        marker
                            .width(Length::Px(f32::from(source.indent_px)))
                            .height(Length::Px(s.line_height))
                            .shrink(0.0),
                    )
                    .child(
                        content
                            .text_role(crate::layout::TextRole::Prose)
                            .grow(1.0)
                            .min_width(Length::Px(0.0)),
                    );
            }
            content
        };
        let content = if compact && row.author == Some(Voice::User) {
            Node::row().child(s.label("You · ", s.muted)).child(
                content
                    .grow(1.0)
                    .basis(Length::Px(0.0))
                    .min_width(Length::Px(0.0)),
            )
        } else {
            content
        };
        node.child(
            content
                .id(Widget::stage_row(visible))
                .height(Length::Px(body_height as f32))
                .shrink(0.0)
                .viewport(),
        )
        .child(S::vspace(row.after as f32))
    }

    fn edit_actions(s: &S, line: &crate::turn::Line, visible: usize) -> Node {
        let action = |label, id| {
            Node::row()
                .child(S::hspace(12.0))
                .child(s.label(label, s.accent))
                .child(S::hspace(12.0))
                .id(id)
                .shrink(0.0)
        };
        let mut controls = Node::row()
            .child(S::hspace(8.0))
            .child(action("Open", Widget::edit_open(visible)));
        if matches!(line.kind(), LineKind::Edit { undone: false, .. }) {
            controls = controls.child(action("Undo", Widget::edit_undo(visible)));
        }
        controls
    }

    fn card(
        model: &Model,
        s: &S,
        layout: &super::FlightLayout,
        line: &crate::turn::Line,
        height: usize,
    ) -> Node {
        let compact = layout.conversation_height < 380.0;
        let mut card =
            ConversationCard::of(model.turn(), line).expect("a measured card has a source record");
        if card.running && model.turn_state() != super::TurnState::Streaming {
            card.status = "No result reported".into();
        }
        let width = (layout.left_width - 42.0).max(1.0);
        let mut face = Typography::default();
        face.set_cell(model.cell_size());
        let (title_height, detail_height) = card.heights(&face, width);
        let status_width = card.status_width(&face);
        let color = if card.failed {
            Pen::for_theme(&model.theme()).error.fg
        } else if card.running {
            s.accent
        } else {
            s.good
        };
        let selected = match line.kind() {
            LineKind::Tool { id, .. } | LineKind::Edit { id, .. } => {
                model.selected() == Some(&Selected::Tool(id.clone()))
            }
            _ => false,
        };
        let background = if selected { s.selected } else { s.raised };
        let title = Node::row()
            .height(Length::Px(title_height as f32))
            .shrink(0.0)
            .child(
                s.heading(card.title)
                    .grow(1.0)
                    .basis(Length::Px(0.0))
                    .min_width(Length::Px(0.0)),
            )
            .child(S::hspace(20.0))
            .child(
                s.label(card.status, color)
                    .width(Length::Px(status_width))
                    .shrink(0.0),
            );
        let mut content = Node::column().min_height(Length::Px(0.0)).child(title);
        if detail_height > 0 && !compact {
            content = content.child(S::vspace(6.0)).child(
                s.prose(card.detail)
                    .style(s.style(s.muted))
                    .height(Length::Px(detail_height as f32))
                    .shrink(0.0),
            );
        }
        s.ground(
            s.inset(content, 20.0, if compact { 4.0 } else { 12.0 }),
            background,
        )
        .height(Length::Px(height as f32))
    }
}
