//! One object card, in one place, for every terminal view.

use super::marked::Marked;
use super::space::Space;
use super::{Model, Pane, Selected, Widget};
use crate::layout::{Frame, Length, Node, TextMetrics};
use crate::pixel::Icon;
use crate::render::Style;
use crate::theme::Pen;
use crate::turn::diff_kind;
use crate::widget_id::WidgetId;

/// Inspector rendering and its scroll extent share the same card rows.
pub struct Inspector;

struct Card {
    title: String,
    status: String,
    icon: Icon,
    metadata: String,
    actions: Vec<(&'static str, WidgetId)>,
    fields: Vec<(String, String)>,
    diff: Option<String>,
    proposals: Vec<(usize, String)>,
}

impl Inspector {
    /// Render the selected object with a fixed header and scrolling fields.
    #[must_use]
    pub fn view(model: &Model, pen: &Pen) -> Node {
        let mut panel = Node::column()
            .id(Widget::Inspector.id())
            .style(pen.stage.clone())
            .viewport()
            .min_width(Length::Cells(0))
            .frame(
                Frame::new(pen.panel(model.pane() == Pane::Inspector).clone())
                    .title("inspector")
                    .title_style(pen.heading.clone())
                    .drawn(model.has_pixels()),
            );
        if model.inspector_stacked() {
            panel = panel
                .height(Length::Cells(model.inspector_stack_rows()))
                .shrink(0.0);
        } else {
            panel = panel
                .width(Length::Cells(Space::INSPECTOR_COLS))
                .shrink(0.0);
        }
        let Some(card) = Card::of(model) else {
            let text = if model.selected().is_some() {
                "Object no longer available"
            } else {
                "Select a work item, step or change"
            };
            return panel.child(line(text, &pen.note));
        };
        let width = model.inspector_width();
        let actions = card
            .actions
            .iter()
            .fold(Node::row().gap(Space::TIGHT), |row, (label, id)| {
                row.child(
                    line(
                        label,
                        if *label == "Undo" {
                            &pen.deny
                        } else {
                            &pen.link
                        },
                    )
                    .id(*id),
                )
            })
            .child(
                line(
                    if model.inspector_open() {
                        "Less"
                    } else {
                        "More"
                    },
                    &pen.permission,
                )
                .id(Widget::InspectorDetail.id()),
            );
        let fields = card.rows(width, model.inspector_open(), pen);
        let height = Self::field_height(model);
        let top = model
            .inspector_scroll
            .min(fields.len().saturating_sub(height));
        // A horizontal gutter only: the stacked card's five inner rows remain usable.
        let content = Node::column()
            .grow(1.0)
            .min_width(Length::Cells(0))
            .viewport()
            .child(line(&card.title, &pen.heading).id(Widget::InspectorTitle.id()))
            .child(
                Marked::new(model, pen)
                    .icon(card.icon)
                    .text(&format!(" {}", card.status))
                    .node()
                    .style(pen.stage.clone())
                    .height(Length::Cells(1))
                    .shrink(0.0)
                    .id(Widget::InspectorStatus.id()),
            )
            .child(line(&card.metadata, &pen.note).id(Widget::InspectorMetadata.id()))
            .child(
                actions
                    .height(Length::Cells(1))
                    .shrink(0.0)
                    .viewport()
                    .id(Widget::InspectorActions.id()),
            )
            .child(
                Node::column()
                    .grow(1.0)
                    .viewport()
                    .id(Widget::InspectorFields.id())
                    .children(fields.into_iter().skip(top).take(height)),
            );
        panel.child(
            Node::row()
                .grow(1.0)
                .viewport()
                .child(Node::leaf().width(Length::Cells(Space::PAD)).shrink(0.0))
                .child(content)
                .child(Node::leaf().width(Length::Cells(Space::PAD)).shrink(0.0)),
        )
    }

    /// All hit targets rendered inside this card, including its domain actions.
    pub(super) fn contains_target(id: WidgetId) -> bool {
        matches!(
            Widget::from_id(id),
            Some(
                Widget::Inspector
                    | Widget::InspectorTitle
                    | Widget::InspectorStatus
                    | Widget::InspectorMetadata
                    | Widget::InspectorActions
                    | Widget::InspectorFields
                    | Widget::InspectorDetail
                    | Widget::RoadmapStart
                    | Widget::RoadmapComplete
            )
        ) || Widget::trace_pin_step(id).is_some()
            || Widget::trace_link_step(id).is_some()
            || Widget::trace_chunk_step(id).is_some()
            || Widget::change_open_index(id).is_some()
            || Widget::change_undo_index(id).is_some()
            || Widget::change_approve_index(id).is_some()
            || Widget::proposal_accept_index(id).is_some()
            || Widget::proposal_reject_index(id).is_some()
            || Widget::signal_promote_index(id).is_some()
            || Widget::signal_ignore_index(id).is_some()
            || Widget::signal_snooze_index(id).is_some()
    }

    /// Scroll within the current field set, clamped to its visible extent.
    pub fn scroll(model: &mut Model, delta: isize) {
        let pen = Pen::for_theme(&model.theme());
        let total = Card::of(model).map_or(0, |card| {
            card.rows(model.inspector_width(), model.inspector_open(), &pen)
                .len()
        });
        let max = total.saturating_sub(Self::field_height(model));
        model.inspector_scroll = model.inspector_scroll.saturating_add_signed(delta).min(max);
    }

    fn field_height(model: &Model) -> usize {
        if model.inspector_stacked() {
            usize::from(model.inspector_stack_rows()).saturating_sub(6)
        } else {
            // Stage adds a tab row and (on graphics) vertical padding.
            model
                .stage_rows()
                .saturating_add(
                    1 + if model.has_pixels() {
                        2 * Space::PAD as usize
                    } else {
                        0
                    },
                )
                .saturating_sub(4)
        }
    }
}

impl Card {
    fn of(model: &Model) -> Option<Self> {
        let mut card = Self {
            title: String::new(),
            status: String::new(),
            icon: Icon::Pending,
            metadata: String::new(),
            actions: Vec::new(),
            fields: Vec::new(),
            diff: None,
            proposals: Vec::new(),
        };
        match model.selected()? {
            Selected::Chunk(id) => {
                let chunk = model.roadmap().chunk(id)?;
                card.title = chunk.name.clone();
                card.status = chunk.status.replace('_', " ");
                card.icon = match chunk.status.as_str() {
                    "done" => Icon::Done,
                    "in_progress" => Icon::InProgress,
                    "blocked" | "stuck" => Icon::Blocked,
                    _ => Icon::Pending,
                };
                card.metadata = format!(
                    "{} · {}",
                    chunk.band(),
                    chunk.group.as_deref().unwrap_or("work item")
                );
                card.fields = vec![
                    ("title".into(), chunk.title.clone()),
                    ("description".into(), chunk.description.clone()),
                ];
                if model.inspector_open() {
                    card.fields.push(("id".into(), chunk.id.clone()));
                    card.fields.extend(
                        chunk
                            .acceptance
                            .iter()
                            .enumerate()
                            .map(|(i, text)| (format!("acceptance {}", i + 1), text.clone())),
                    );
                    card.fields
                        .push(("depends on".into(), chunk.deps.join(", ")));
                    card.fields
                        .push(("references".into(), chunk.cross_refs.join(", ")));
                }
                card.proposals = model
                    .roadmap()
                    .proposals()
                    .into_iter()
                    .enumerate()
                    .filter(|(_, (owner, _, _, _))| owner.id == chunk.id)
                    .map(|(index, (_, kind, suggestion, reason))| {
                        (index, format!("{} → {suggestion} · {reason}", kind.label()))
                    })
                    .collect();
                if matches!(chunk.status.as_str(), "in_progress" | "stuck") {
                    card.actions
                        .push(("Complete", Widget::RoadmapComplete.id()));
                } else if matches!(chunk.status.as_str(), "pending" | "blocked" | "backlog") {
                    card.actions.push(("Start", Widget::RoadmapStart.id()));
                }
            }
            Selected::Signal(id) => {
                let signals = model.roadmap().signals();
                let (index, signal) = signals
                    .iter()
                    .enumerate()
                    .find(|(_, signal)| signal.id == *id)?;
                card.title = signal.title().to_owned();
                card.status = signal.status.clone();
                card.icon = Icon::Signal;
                card.metadata = format!("{} · {}", signal.kind, signal.from);
                card.actions = vec![
                    ("Promote", Widget::signal_promote(index)),
                    ("Ignore", Widget::signal_ignore(index)),
                    ("Snooze", Widget::signal_snooze(index)),
                ];
                card.fields = vec![("body".into(), signal.body.clone())];
                if let Some(findings) = &signal.findings {
                    card.fields.push(("findings".into(), findings.clone()));
                }
                if model.inspector_open() {
                    card.fields.push(("id".into(), signal.id.clone()));
                    if let Some(confidence) = signal.confidence {
                        card.fields
                            .push(("confidence".into(), format!("{confidence:.1}")));
                    }
                }
            }
            Selected::Step(number) => {
                let step = model.trace().step(*number)?;
                card.title = if step.purpose.is_empty() {
                    format!("Step {number}")
                } else {
                    step.purpose.clone()
                };
                card.status = if step.revised_by.is_some() {
                    "revised"
                } else if step.is_final {
                    "final"
                } else {
                    "recorded"
                }
                .into();
                card.icon = if step.pinned {
                    Icon::Pinned
                } else {
                    Icon::Done
                };
                card.metadata = format!(
                    "step {number} · {}",
                    step.branch_name
                        .as_deref()
                        .or(step.branch_id.as_deref())
                        .unwrap_or("main")
                );
                card.actions.push((
                    if step.pinned { "Unpin" } else { "Pin" },
                    Widget::trace_pin(*number),
                ));
                if step
                    .execution_ref
                    .as_deref()
                    .is_some_and(|r| model.roadmap().chunk_for_ref(r).is_some())
                {
                    card.actions
                        .push(("Open chunk", Widget::trace_chunk(*number)));
                }
                card.fields = step
                    .fields()
                    .into_iter()
                    .filter(|(label, _)| {
                        model.inspector_open()
                            || matches!(*label, "thought" | "outcome" | "next action")
                    })
                    .map(|(label, value)| (label.into(), value))
                    .collect();
                if let Some(n) = step.revises {
                    card.fields.push(("revises".into(), n.to_string()));
                }
                if let Some(n) = step.revised_by {
                    card.fields.push(("revised by".into(), n.to_string()));
                }
            }
            Selected::Change(id) => {
                let (index, entry) = model
                    .changes()
                    .entries()
                    .iter()
                    .enumerate()
                    .find(|(_, entry)| entry.id == *id)?;
                card.title = entry.path.clone();
                card.status = if entry.parse_ok {
                    "parse ok"
                } else {
                    "parse failed"
                }
                .into();
                if model.changes().is_approved(id) {
                    card.status.push_str(" · approved");
                }
                card.icon = Icon::check(entry.parse_ok);
                card.metadata = format!("+{} −{} · {}", entry.added, entry.removed, entry.id);
                card.actions = vec![
                    ("Open", Widget::change_open(index)),
                    ("Undo", Widget::change_undo(index)),
                    ("Approve", Widget::change_approve(index)),
                ];
                card.fields = vec![("path".into(), entry.path.clone())];
                card.diff = Some(entry.diff.clone());
            }
        }
        Some(card)
    }

    fn rows(&self, width: u16, full: bool, pen: &Pen) -> Vec<Node> {
        let mut rows = Vec::new();
        for (index, proposal) in &self.proposals {
            rows.extend(
                TextMetrics::new(proposal)
                    .wrap(width.max(1))
                    .into_iter()
                    .map(|text| line(text, &pen.permission)),
            );
            rows.push(
                Node::row()
                    .gap(Space::TIGHT)
                    .height(Length::Cells(1))
                    .shrink(0.0)
                    .child(line("Accept", &pen.allow).id(Widget::proposal_accept(*index)))
                    .child(line("Reject", &pen.deny).id(Widget::proposal_reject(*index))),
            );
        }
        for (label, value) in &self.fields {
            if value.trim().is_empty() {
                continue;
            }
            let text = format!("{label} · {value}");
            let metrics = TextMetrics::new(text);
            let wrapped = metrics.wrap(width.max(1));
            let limit = if full { wrapped.len() } else { 2 };
            rows.extend(wrapped.into_iter().take(limit).map(|text| {
                let row = line(text, &pen.stage);
                if matches!(label.as_str(), "revises" | "revised by") {
                    value.parse().map_or(row.clone(), |number| {
                        row.id(Widget::trace_link(number)).style(pen.link.clone())
                    })
                } else {
                    row
                }
            }));
        }
        if let Some(diff) = &self.diff {
            for text in diff.lines().take(if full { usize::MAX } else { 2 }) {
                rows.extend(
                    TextMetrics::new(text)
                        .wrap(width.max(1))
                        .into_iter()
                        .map(|wrapped| line(wrapped, pen.row(diff_kind(text)))),
                );
            }
        }
        rows
    }
}

fn line(text: &str, style: &Style) -> Node {
    Node::text(text)
        .style(style.clone())
        .height(Length::Cells(1))
        .shrink(0.0)
}
