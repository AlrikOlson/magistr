//! Roadmap requests projected from the real conversation, with their own scroll position.
use super::{FlightStyle as S, Model, RequestPhase, Widget};
use crate::layout::{Length, Node, TextMetrics, TextRole};
use crate::pixel::Typography;
use crate::render::Color;
use crate::turn::{ConversationCard, LineKind, Voice};

/// A request and its replies stay beside the plan they concern.
pub struct FlightRequests;

struct Row {
    node: Node,
    height: f32,
}

struct Rows<'a> {
    style: &'a S,
    face: Typography,
    width: f32,
    rows: Vec<Row>,
}

impl<'a> Rows<'a> {
    fn new(model: &Model, style: &'a S, width: f32) -> Self {
        let mut face = Typography::default();
        face.set_cell(model.cell_size());
        Self {
            style,
            face,
            width: width.max(1.0),
            rows: Vec::new(),
        }
    }

    fn text(&mut self, text: &str, role: TextRole, ink: Color) {
        if text.is_empty() {
            self.gap(10.0);
            return;
        }
        for line in TextMetrics::new(text).wrap_by(self.width, &mut |s| self.face.advance(s, role))
        {
            let node = if role == TextRole::Heading {
                self.style.heading(line)
            } else {
                Node::text(line)
                    .text_role(role)
                    .style(self.style.style(ink))
            };
            self.rows.push(Row {
                node: node.height(Length::Px(self.style.line_height)).shrink(0.0),
                height: self.style.line_height,
            });
        }
    }

    fn gap(&mut self, height: f32) {
        self.rows.push(Row {
            node: S::vspace(height),
            height,
        });
    }
}

impl FlightRequests {
    #[must_use]
    pub fn view(model: &Model, width: f32, height: f32) -> Node {
        let s = S::for_model(model);
        let body_height = (height - s.line_height - 40.0).max(0.0);
        let rows = rows(model, &s, width - 34.0);
        let top = model
            .roadmap_request_scroll()
            .min(max_top(&rows, body_height));
        let body = Node::column()
            .id(Widget::RequestBody.id())
            .viewport()
            .height(Length::Px(body_height))
            .grow(1.0)
            .min_height(Length::Px(0.0))
            .children(
                rows.into_iter()
                    .skip(top)
                    .scan(body_height, |remaining, row| {
                        if row.height > *remaining {
                            return None;
                        }
                        *remaining -= row.height;
                        Some(row.node)
                    }),
            );
        let aside = model.roadmap_requests().last().map_or("Plan changes", |r| {
            if r.phase == RequestPhase::Queued {
                "Queued"
            } else {
                r.phase.label()
            }
        });
        let aside = if model.roadmap_request_scroll() != usize::MAX
            && !model.roadmap_requests().is_empty()
        {
            s.label("Latest reply ↓", s.accent)
                .id(Widget::RequestLatest.id())
        } else {
            s.label(aside, s.accent)
        };
        let mut panel = s
            .outlined(
                Node::column()
                    .min_height(Length::Px(0.0))
                    .child(s.section_heading("Requests", aside))
                    .child(s.inset(body, 16.0, 12.0).grow(1.0)),
                s.panel,
                s.line,
            )
            .height(Length::Px(height))
            .min_height(Length::Px(0.0));
        if model.hovered() == Some(Widget::RequestLatest.id()) {
            panel.restyle_at(Widget::RequestLatest.id(), &|style| {
                style.clone().bg(s.selected)
            });
        }
        panel
    }

    pub fn scroll(model: &mut Model, delta: isize) {
        let s = S::for_model(model);
        let l = model.flight_layout();
        let width = if l.compact { l.width } else { l.right_width };
        let height = super::FlightRoadmapPage::detail_height(model);
        let rows = rows(model, &s, width - 34.0);
        let max = max_top(&rows, (height - s.line_height - 40.0).max(0.0));
        let top = model
            .roadmap_request_scroll
            .min(max)
            .saturating_add_signed(delta)
            .min(max);
        model.roadmap_request_scroll = if top == max && delta > 0 {
            usize::MAX
        } else {
            top
        };
    }
}

fn max_top(rows: &[Row], height: f32) -> usize {
    let mut extent = 0.0;
    for (index, row) in rows.iter().enumerate().rev() {
        if extent + row.height > height {
            return (index + 1).min(rows.len().saturating_sub(1));
        }
        extent += row.height;
    }
    0
}

fn rows(model: &Model, s: &S, width: f32) -> Vec<Row> {
    let mut out = Rows::new(model, s, width);
    if model.roadmap_requests().is_empty() {
        out.text("What should change?", TextRole::Heading, s.ink);
        out.gap(8.0);
        out.text("Ask to add work, change priorities, or revise the plan. Your request and Magistr’s reply will stay here.", TextRole::Prose, s.ink);
        out.gap(16.0);
        out.text(
            "Updating the roadmap does not start an item.",
            TextRole::Prose,
            s.muted,
        );
        return out.rows;
    }
    for (index, request) in model.roadmap_requests().iter().enumerate() {
        if index > 0 {
            out.gap(28.0);
        }
        out.text("You", TextRole::Heading, s.ink);
        out.gap(8.0);
        out.text(&request.text, TextRole::Prose, s.ink);
        out.gap(18.0);
        let latest = index + 1 == model.roadmap_requests().len();
        let status = if latest && request.phase == RequestPhase::Working {
            model.agent_activity().status
        } else {
            request.phase.label()
        };
        out.text(&format!("Magistr · {status}"), TextRole::Heading, s.accent);
        out.gap(8.0);
        if request.phase == RequestPhase::Queued {
            out.text("This request will start after the current work item finishes. You can cancel it below.", TextRole::Prose, s.muted);
        } else if request.phase == RequestPhase::Waiting {
            out.text(
                "Your request has been sent. Waiting for the agent to respond.",
                TextRole::Prose,
                s.muted,
            );
        }
        if let Some(start) = request.source_start {
            let end = request.source_end.unwrap_or(model.turn().lines().len());
            for line in model.turn().lines().iter().take(end).skip(start) {
                match line.kind() {
                    LineKind::Prose(Voice::User | Voice::Thought) | LineKind::Event => {}
                    LineKind::Tool { .. } | LineKind::Edit { .. } | LineKind::Check { .. } => {
                        if let Some(card) = ConversationCard::of(model.turn(), line) {
                            out.gap(8.0);
                            out.text(&card.title, TextRole::Heading, s.ink);
                            out.text(&card.status, TextRole::Label, s.muted);
                            out.gap(8.0);
                        }
                    }
                    _ => out.text(line.text(), TextRole::Prose, s.ink),
                }
            }
        }
        if let Some(error) = &request.error {
            out.text(
                error,
                TextRole::Prose,
                crate::theme::Pen::for_theme(&model.theme()).error.fg,
            );
            out.gap(8.0);
            out.text(
                if model.prompt().text() == request.text {
                    "Your draft is below. You can edit it and send it again."
                } else if model.prompt().text().is_empty() {
                    "Your request is saved above. You can send it again below."
                } else {
                    "Your newer draft is kept. The earlier request is saved above."
                },
                TextRole::Prose,
                s.muted,
            );
        }
    }
    out.rows
}
