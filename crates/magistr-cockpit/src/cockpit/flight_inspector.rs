//! Selected records in the Flight Recorder's independent details area.

use super::{Card, Inspector};
use crate::cockpit::session_item::SessionItem;
use crate::cockpit::{FlightStyle, Model, Pane, Selected, ShipMoment, TraceStep, Widget};
use crate::layout::{Length, Node, TextMetrics, TextRole};
use crate::pixel::Typography;
use crate::render::{Attrs, Color};
use crate::theme::Pen;
use crate::widget_id::WidgetId;

struct Row {
    node: Node,
    height: f32,
}

struct Rows<'a> {
    style: &'a FlightStyle,
    face: Typography,
    width: f32,
    line: f32,
    rows: Vec<Row>,
}

impl<'a> Rows<'a> {
    fn new(model: &Model, style: &'a FlightStyle, width: f32) -> Self {
        let mut face = Typography::default();
        face.set_cell(model.cell_size());
        Self {
            style,
            face,
            width: width.max(1.0),
            line: f32::from(model.cell_size().height),
            rows: Vec::new(),
        }
    }

    fn text(&mut self, text: &str, role: TextRole, ink: Color, id: Option<WidgetId>) {
        if text.is_empty() {
            return;
        }
        let metrics = TextMetrics::new(text);
        for text in metrics.wrap_by(self.width, &mut |s| self.face.advance(s, role)) {
            let mut node = Node::text(text)
                .text_role(role)
                .style(self.style.style(ink))
                .height(Length::Px(self.line))
                .shrink(0.0);
            if role == TextRole::Heading {
                node = node.style(self.style.style(ink).attrs(Attrs::BOLD));
            }
            if let Some(id) = id {
                node = node.id(id);
            }
            self.rows.push(Row {
                node,
                height: self.line,
            });
        }
    }

    fn gap(&mut self, height: f32) {
        self.rows.push(Row {
            node: FlightStyle::vspace(height),
            height,
        });
    }

    fn field(&mut self, label: &str, value: &str) {
        if value.trim().is_empty() {
            return;
        }
        self.gap(16.0);
        self.text(label, TextRole::Heading, self.style.ink, None);
        self.gap(4.0);
        self.text(value, TextRole::Prose, self.style.ink, None);
    }
}

impl Inspector {
    /// A linked work name and the exact recorded object it opens.
    #[must_use]
    pub fn linked_work(model: &Model, number: u32) -> Option<(String, Selected)> {
        let reference = model.trace().step(number)?.execution_ref.as_deref()?;
        if let Some(id) = reference.strip_prefix("task:")
            && let Some(task) = model.rail().ship().task(id)
        {
            return Some((task.title.clone(), Selected::Task(task.id.clone())));
        }
        let mut matches = SessionItem::collect(model.trace(), model.changes())
            .into_iter()
            .filter_map(|item| {
                let selection = item.selection();
                let Selected::Receipt(identity) = &selection else {
                    return None;
                };
                let (label, aliases) = match item {
                    SessionItem::Moment(
                        _,
                        ShipMoment::Action {
                            task_id,
                            id,
                            description,
                            ..
                        },
                    ) => (
                        description.clone(),
                        vec![format!("action:{task_id}:{id}"), format!("action:{id}")],
                    ),
                    SessionItem::Moment(_, ShipMoment::Check { task_id, name, .. }) => (
                        name.clone(),
                        vec![format!("check:{task_id}:{name}"), format!("check:{name}")],
                    ),
                    _ => return None,
                };
                (reference == identity || aliases.iter().any(|alias| alias == reference))
                    .then_some((label, selection))
            });
        let first = matches.next();
        if first.is_some() && matches.next().is_none() {
            return first;
        }
        model.roadmap().chunk_for_ref(reference).map(|chunk| {
            (
                if chunk.title.is_empty() {
                    chunk.name.clone()
                } else {
                    chunk.title.clone()
                },
                Selected::Chunk(chunk.id.clone()),
            )
        })
    }

    /// A bounded details area with the same frame and independent scrolling for every record.
    #[must_use]
    pub fn flight_view(model: &Model, width: f32, height: f32) -> Node {
        let style = FlightStyle::for_model(model);
        let Some(card) = Card::of(model) else {
            let content = Node::column()
                .child(style.section_heading("Details", Node::leaf()))
                .child(style.inset(
                    style.prose(if model.selected().is_some() {
                        "This record is no longer available."
                    } else {
                        "Select a step or work item to see its details."
                    }),
                    16.0,
                    12.0,
                ));
            return style
                .outlined(content, style.panel, style.line)
                .id(Widget::Inspector.id());
        };
        let step = match model.selected() {
            Some(Selected::Step(number)) if card.viewer.is_none() => model.trace().step(*number),
            _ => None,
        };
        let title = step.map_or_else(|| "Details".to_owned(), |s| format!("Step {:02}", s.number));
        let latest = step.is_some_and(|s| {
            model
                .trace()
                .steps()
                .last()
                .is_some_and(|last| last.number == s.number)
        });
        let mut aside = style
            .label(
                if model.inspector_open() {
                    "Less"
                } else if latest {
                    "Latest · More"
                } else if step.is_some() {
                    "Selected · More"
                } else {
                    "More"
                },
                style.accent,
            )
            .id(Widget::InspectorDetail.id());
        if Inspector::full_roadmap_details(model) && card.viewer.is_none() {
            aside = style.label("Work item", style.muted);
        }
        if model.tab() == crate::cockpit::StageTab::Roadmap && model.flight_layout().compact {
            aside = style
                .label("Back to roadmap", style.accent)
                .id(Widget::TabRoadmap.id());
        }
        let rows = body(model, &style, &card, step, width - 34.0);
        let footer = footer(model, &style, &card, step, width - 34.0);
        let body_height = (height - 48.0 - style.line_height * 2.0).max(0.0);
        let top = model.inspector_scroll.min(max_top(&rows, body_height));
        let fields = Node::column()
            .id(Widget::InspectorFields.id())
            .viewport()
            .height(Length::Px(body_height))
            .grow(1.0)
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
        let content = Node::column()
            .min_height(Length::Px(0.0))
            .child(style.section_heading(title, aside))
            .child(
                style
                    .inset(
                        Node::column()
                            .min_height(Length::Px(0.0))
                            .child(fields)
                            .child(FlightStyle::vspace(8.0))
                            .child(footer),
                        16.0,
                        10.0,
                    )
                    .grow(1.0),
            );
        let mut panel = style
            .outlined(
                content,
                style.panel,
                if model.pane() == Pane::Inspector {
                    style.accent
                } else {
                    style.line
                },
            )
            .id(Widget::Inspector.id())
            .min_height(Length::Px(0.0));
        if let Some(id) = model.hovered()
            && crate::cockpit::HoverRole::of(id).is_clickable()
        {
            panel.restyle_at(id, &|s| s.clone().bg(style.selected));
        }
        panel
    }

    /// Scroll measured lines without moving the conversation or the message draft.
    pub fn flight_scroll(model: &mut Model, delta: isize) {
        let Some(card) = Card::of(model) else {
            return;
        };
        let style = FlightStyle::for_model(model);
        let step = match model.selected() {
            Some(Selected::Step(number)) if card.viewer.is_none() => model.trace().step(*number),
            _ => None,
        };
        let layout = model.flight_layout();
        let (width, height) = if model.tab() == crate::cockpit::StageTab::Roadmap {
            (
                if layout.compact {
                    layout.width
                } else {
                    layout.right_width
                },
                crate::cockpit::FlightRoadmapPage::detail_height(model),
            )
        } else if layout.compact {
            (layout.width, layout.body_height)
        } else {
            (layout.right_width, layout.detail_height)
        };
        let rows = body(model, &style, &card, step, width - 34.0);
        let max = max_top(&rows, (height - 48.0 - style.line_height * 2.0).max(0.0));
        model.inspector_scroll = model.inspector_scroll.saturating_add_signed(delta).min(max);
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

fn body(
    model: &Model,
    style: &FlightStyle,
    card: &Card,
    step: Option<&TraceStep>,
    width: f32,
) -> Vec<Row> {
    let mut rows = Rows::new(model, style, width);
    rows.text(&card.title, TextRole::Heading, style.ink, None);
    rows.gap(4.0);
    if let Some(step) = step {
        let mut relation = Vec::new();
        if let Some(number) = step.revises {
            relation.push(format!("Updates step {number}"));
        }
        if let Some(name) = step.branch_name.as_deref().filter(|s| !s.is_empty()) {
            relation.push(name.to_owned());
        } else if let Some(confidence) = step.confidence {
            relation.push(format!("{:.0}% confidence", confidence * 100.0));
        }
        if step.pinned {
            relation.push("Saved".to_owned());
        }
        rows.text(
            &relation.join(" · "),
            TextRole::Label,
            style.muted,
            step.revises.map(Widget::trace_link),
        );
        rows.gap(8.0);
        rows.text(&step.thought, TextRole::Prose, style.ink, None);
        rows.gap(8.0);
        rows.text(
            &if step.outcome.is_empty() {
                "No result recorded yet.".into()
            } else {
                format!("Result: {}", step.outcome)
            },
            TextRole::Prose,
            style.ink,
            None,
        );
        rows.field(
            "Why",
            if step.rationale.is_empty() {
                "No reason recorded."
            } else {
                &step.rationale
            },
        );
        rows.field(
            "Next",
            if step.next_action.is_empty() {
                "No next action recorded."
            } else {
                &step.next_action
            },
        );
        if let Some(number) = step.revised_by {
            rows.gap(16.0);
            rows.text(
                &format!("Updated by step {number} →"),
                TextRole::Label,
                style.accent,
                Some(Widget::trace_link(number)),
            );
        }
        if model.inspector_open() {
            rows.field("Context", &step.context);
            rows.field("Uncertainty", &step.uncertainty);
            rows.field("Reason for the update", &step.revision_reason);
            if let Some(timestamp) = &step.timestamp {
                rows.field("Recorded", timestamp);
            }
            for number in &step.dependencies {
                rows.gap(8.0);
                rows.text(
                    &format!("Uses step {number} →"),
                    TextRole::Label,
                    style.accent,
                    Some(Widget::trace_link(*number)),
                );
            }
            for investigation in model
                .trace()
                .investigations()
                .into_iter()
                .filter(|b| b.contains(step.number))
            {
                rows.field("Question", investigation.question);
                rows.field(
                    "Investigation",
                    &format!(
                        "{} · {}",
                        investigation.status_label(),
                        investigation.relationship()
                    ),
                );
            }
        }
    } else {
        rows.text(&card.status, TextRole::Label, style.accent, None);
        rows.gap(4.0);
        rows.text(&card.metadata, TextRole::Label, style.muted, None);
        if model.tab() == crate::cockpit::StageTab::Roadmap
            && matches!(model.selected(), Some(Selected::Chunk(_)))
        {
            rows.gap(12.0);
            rows.text(
                "Request a change →",
                TextRole::Label,
                style.accent,
                Some(Widget::RoadmapRequest.id()),
            );
        }
        if let Some(lines) = &card.viewer {
            for line in lines {
                rows.text(line, TextRole::Code, style.ink, None);
            }
        } else {
            for (label, value) in &card.fields {
                rows.field(label, value);
            }
            if Inspector::full_roadmap_details(model)
                && let Some(Selected::Chunk(id)) = model.selected()
                && let Some(chunk) = model.roadmap().chunk(id)
            {
                if chunk.deps.is_empty() {
                    rows.field("Depends on", "No dependencies");
                } else {
                    rows.gap(16.0);
                    rows.text("Depends on", TextRole::Heading, style.ink, None);
                    rows.gap(4.0);
                    for (index, reference) in chunk.deps.iter().enumerate() {
                        if let Some(dependency) = model.roadmap().chunk_for_ref(reference) {
                            rows.text(
                                &format!("{} →", super::work_name(dependency)),
                                TextRole::Label,
                                style.accent,
                                Some(Widget::roadmap_dependency(index)),
                            );
                        } else {
                            rows.text("Unavailable work item", TextRole::Label, style.muted, None);
                        }
                    }
                }
            }
            for (index, text) in &card.proposals {
                rows.field("Suggested change", text);
                rows.gap(8.0);
                rows.text(
                    "Accept",
                    TextRole::Label,
                    style.good,
                    Some(Widget::proposal_accept(*index)),
                );
                rows.text(
                    "Reject",
                    TextRole::Label,
                    style.accent,
                    Some(Widget::proposal_reject(*index)),
                );
            }
            if let Some(diff) = &card.diff {
                rows.gap(12.0);
                let pen = Pen::for_theme(&model.theme());
                for line in diff.lines() {
                    rows.text(
                        line,
                        TextRole::Code,
                        pen.row(crate::turn::diff_kind(line)).fg,
                        None,
                    );
                }
            }
        }
    }
    if step.is_some() && model.inspector_open() {
        rows.gap(16.0);
        for (label, id) in &card.actions {
            rows.text(label, TextRole::Label, style.accent, Some(*id));
            rows.gap(8.0);
        }
    }
    rows.rows
}

fn footer(
    model: &Model,
    style: &FlightStyle,
    card: &Card,
    step: Option<&TraceStep>,
    width: f32,
) -> Node {
    let mut row = Node::row()
        .height(Length::Px(style.line_height + 4.0))
        .shrink(0.0)
        .viewport();
    if let Some(step) = step {
        if let Some((name, _)) = Inspector::linked_work(model, step.number) {
            let mut face = Typography::default();
            face.set_cell(model.cell_size());
            let label = FlightStyle::fit_label(
                &format!("Work: {name}"),
                (width - 24.0 - face.advance("  →", TextRole::Label)).max(1.0),
                &face,
            );
            row = row.child(
                style
                    .chip(format!("{label}  →"), style.accent, style.raised)
                    .id(Widget::LinkedWork.id()),
            );
        } else {
            row = row.child(style.label(
                if step.execution_ref.is_some() {
                    "Linked work is no longer available."
                } else {
                    "No linked work"
                },
                style.muted,
            ));
        }
        return row.child(FlightStyle::flex());
    }
    for (label, id) in &card.actions {
        row = row
            .child(FlightStyle::hspace(8.0))
            .child(style.chip(*label, style.accent, style.raised).id(*id));
    }
    row.child(FlightStyle::flex())
}

#[cfg(test)]
#[path = "flight_inspector.tests.rs"]
mod tests;
