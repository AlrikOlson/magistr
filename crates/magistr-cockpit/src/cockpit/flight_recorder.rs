//! The live reasoning recorder: one source of geometry for labels, drawing and input.

use super::flight_style::FlightStyle;
use super::trace::Investigation;
use super::{Model, Selected, Widget};
use crate::layout::{Align, Inset, Length, Node, TextRole};
use crate::pixel::{BranchGraph, Typography};
use crate::render::{Attrs, Color};

/// A bounded view of recorded decisions beneath the conversation.
pub struct FlightRecorder;

impl FlightRecorder {
    /// The full recorder height. Narrow windows show one question at a time.
    #[must_use]
    pub fn height(model: &Model, _width_px: f32) -> f32 {
        280.0_f32.max(f32::from(model.cell_size().height) * 12.0 + 40.0)
    }

    #[must_use]
    pub fn view(model: &Model, width_px: f32) -> Node {
        Self::view_in(model, width_px, Self::height(model, width_px))
    }

    /// Render into the room left after reserving conversation and message space.
    #[must_use]
    pub fn view_in(model: &Model, width_px: f32, height_px: f32) -> Node {
        let style = FlightStyle::for_model(model);
        let graph_width = (width_px - 40.0).max(1.0);
        let graph_height = Self::graph_height(model, graph_width);
        let line = f32::from(model.cell_size().height);
        let trace = model.trace();
        let indices = Self::step_indices(model, graph_width);
        let selected = selected_number(model);
        let mut aside = Node::row().align_items(Align::Center);
        if indices.len() < trace.steps().len() {
            aside = aside
                .child(
                    style
                        .chip("←", style.accent, style.header)
                        .id(Widget::TracePrevious.id()),
                )
                .child(
                    style
                        .chip("→", style.accent, style.header)
                        .id(Widget::TraceNext.id()),
                );
        }
        if !trace.steps().is_empty() {
            let latest = trace.steps().last().map(|s| s.number);
            aside = aside.child(
                style
                    .chip(
                        if selected == latest {
                            "Latest step"
                        } else {
                            "Return to latest →"
                        },
                        style.accent,
                        style.header,
                    )
                    .id(Widget::LatestDecision.id()),
            );
        }
        let mut title = Node::row()
            .align_items(Align::Center)
            .child(style.heading("Reasoning"));
        if width_px >= 650.0 {
            let count = trace.steps().len();
            let questions = trace.branch_count();
            title = title.child(FlightStyle::hspace(16.0)).child(style.label(
                format!(
                    "{count} {} · {questions} {} explored",
                    if count == 1 { "step" } else { "steps" },
                    if questions == 1 {
                        "question"
                    } else {
                        "questions"
                    }
                ),
                style.muted,
            ));
        }
        let header = style
            .ground(
                style.inset(title.child(FlightStyle::flex()).child(aside), 16.0, 0.0),
                style.header,
            )
            .height(Length::Px((line + 14.0).max(34.0)))
            .shrink(0.0);
        let top = if height_px >= 260.0 { 21.0 } else { 8.0 };
        let mut body = Node::column()
            .min_height(Length::Px(0.0))
            .child(FlightStyle::vspace(top));
        if indices.is_empty() {
            body = body
                .child(style.prose(
                    "Reasoning and explored questions will appear here as work progresses.",
                ));
        } else {
            body = body.child(Self::numbered_graph(model, &style, graph_width));
            let remaining = height_px - (line + 14.0).max(34.0) - top - graph_height - line - 29.0;
            if remaining >= line + 24.0 {
                body = body.child(FlightStyle::vspace(12.0)).child(Self::questions(
                    model,
                    &style,
                    graph_width,
                    remaining,
                ));
            }
        }
        let content = Node::column()
            .min_height(Length::Px(0.0))
            .child(header)
            .child(style.inset(body, 19.0, 0.0).grow(1.0).viewport());
        style
            .outlined(
                content,
                style.panel,
                if model.pane() == super::Pane::Rail {
                    style.accent
                } else {
                    style.line
                },
            )
            .height(Length::Px(height_px))
            .shrink(0.0)
            .viewport()
    }

    /// Feed indices in the visible graph. Selection, not global number arithmetic,
    /// chooses the window; every decision remains reachable with previous/next.
    #[must_use]
    pub fn step_indices(model: &Model, graph_width_px: f32) -> Vec<usize> {
        let steps = model.trace().steps();
        if steps.is_empty() {
            return Vec::new();
        }
        let mut typography = Typography::default();
        typography.set_cell(model.cell_size());
        let label = steps
            .iter()
            .map(|step| typography.advance(&format!("{:02}", step.number), TextRole::Label))
            .fold(20.0_f32, f32::max);
        let capacity =
            (((graph_width_px - 72.0).max(0.0) / (label + 22.0)).floor() as usize + 1).max(1);
        let selected = selected_number(model)
            .and_then(|number| steps.iter().position(|step| step.number == number))
            .unwrap_or(steps.len() - 1);
        let start = selected
            .saturating_sub(capacity / 2)
            .min(steps.len().saturating_sub(capacity));
        let mut indices: Vec<_> = (start..(start + capacity).min(steps.len())).collect();
        // Four readable lanes at most. Nested/interleaved branches retain their
        // own lane; surrounding records yield before any lanes collapse together.
        while visible_branches(model, &indices).len() > 3 && indices.len() > 1 {
            if selected.saturating_sub(indices[0])
                > indices[indices.len() - 1].saturating_sub(selected)
            {
                indices.remove(0);
            } else {
                indices.pop();
            }
        }
        indices
    }

    #[must_use]
    pub fn graph_height(model: &Model, _graph_width_px: f32) -> f32 {
        f32::from(model.cell_size().height) * 4.0
    }

    /// The exact graph supplied to the native painter and its pointer registry.
    #[must_use]
    pub fn graph(model: &Model, graph_width_px: f32) -> BranchGraph {
        let trace = model.trace();
        let indices = Self::step_indices(model, graph_width_px);
        let branches = visible_branches(model, &indices);
        let lanes: Vec<_> = indices
            .iter()
            .map(|&index| {
                trace.steps()[index]
                    .branch_id
                    .as_deref()
                    .and_then(|id| branches.iter().position(|known| *known == id))
                    .map_or(0, |lane| (lane + 1) as u8)
            })
            .collect();
        let remap = |edges: Vec<(usize, usize)>| {
            edges
                .into_iter()
                .filter_map(|(from, to)| {
                    Some((
                        indices.iter().position(|i| *i == from)?,
                        indices.iter().position(|i| *i == to)?,
                    ))
                })
                .collect::<Vec<_>>()
        };
        let global_lanes = trace.lanes();
        let style = FlightStyle::for_model(model);
        let lane_color = |lane| {
            let (r, g, b) = rgb(style.lane(lane));
            [r, g, b]
        };
        let colors = std::iter::once(lane_color(0))
            .chain(branches.iter().map(|id| {
                let lane = trace
                    .steps()
                    .iter()
                    .position(|step| step.branch_id.as_deref() == Some(*id))
                    .and_then(|i| global_lanes.get(i))
                    .map_or(0, |(_, lane)| *lane);
                lane_color(lane)
            }))
            .collect();
        let selected = selected_number(model).and_then(|number| {
            indices
                .iter()
                .position(|&i| trace.steps()[i].number == number)
        });
        let (r, g, b) = rgb(style.panel);
        let (ir, ig, ib) = rgb(style.ink);
        BranchGraph::new(lanes, remap(trace.links()))
            .geometry(graph_width_px, Self::graph_height(model, graph_width_px))
            .merges(remap(trace.merges()))
            .pins(
                indices
                    .iter()
                    .map(|&i| trace.steps()[i].pinned)
                    .collect::<Vec<_>>(),
            )
            .focus(None, selected)
            .colors(colors)
            .background(r, g, b)
            .selection_color(ir, ig, ib)
    }

    fn numbered_graph(model: &Model, style: &FlightStyle, width: f32) -> Node {
        let height = Self::graph_height(model, width);
        let line = f32::from(model.cell_size().height);
        let graph = Self::graph(model, width);
        let points = graph.points(width, height);
        let indices = Self::step_indices(model, width);
        let lanes = model.trace().lanes();
        let selected = selected_number(model);
        let mut node = Node::column()
            .height(Length::Px(height + line))
            .shrink(0.0)
            .child(
                Node::leaf()
                    .id(Widget::TraceGraph.id())
                    .height(Length::Px(height))
                    .shrink(0.0)
                    .style(style.style(style.ink)),
            );
        for (&index, (x, y)) in indices.iter().zip(points) {
            let step = &model.trace().steps()[index];
            let active = selected == Some(step.number);
            let ink = if active {
                style.ink
            } else if lanes[index].1 == 0 {
                style.muted
            } else {
                style.lane(lanes[index].1)
            };
            let mut label = style.label(format!("{:02}", step.number), ink);
            if active {
                label = label.style(style.style(ink).attrs(Attrs::BOLD));
            }
            let label_width = (width / indices.len().max(1) as f32).clamp(24.0, 80.0);
            node = node.child(
                Node::column()
                    .align_items(Align::Center)
                    .child(FlightStyle::vspace(20.0))
                    .child(label)
                    .width(Length::Px(label_width))
                    .height(Length::Px(line + 26.0))
                    .overlay(
                        Inset::none()
                            .left(Length::Px(x - label_width / 2.0))
                            .top(Length::Px(y - 10.0)),
                    )
                    .id(Widget::trace_row(index)),
            );
        }
        node
    }

    fn questions(model: &Model, style: &FlightStyle, width: f32, height: f32) -> Node {
        let all = model.trace().investigations();
        if all.is_empty() {
            return style.label("No side questions recorded", style.muted);
        }
        let selected = selected_number(model);
        let preferred = selected
            .and_then(|number| all.iter().position(|q| q.members.contains(&number)))
            .or_else(|| selected.and_then(|number| all.iter().position(|q| q.contains(number))))
            .unwrap_or(all.len() - 1);
        let capacity = if width >= 640.0 { 2 } else { 1 };
        let start = preferred.min(all.len().saturating_sub(capacity));
        let shown = capacity.min(all.len());
        let card_width = (width - 12.0 * shown.saturating_sub(1) as f32) / shown.max(1) as f32;
        let mut row = Node::row().height(Length::Px(height)).shrink(0.0);
        for (at, question) in all.iter().skip(start).take(capacity).enumerate() {
            if at > 0 {
                row = row.child(FlightStyle::hspace(12.0));
            }
            row = row.child(
                question_card(model, style, question, selected, card_width, height)
                    .grow(1.0)
                    .basis(Length::Px(0.0)),
            );
        }
        row
    }
}

fn question_card(
    model: &Model,
    style: &FlightStyle,
    question: &Investigation<'_>,
    selected: Option<u32>,
    width: f32,
    height: f32,
) -> Node {
    let ink = style.lane(question.lane);
    let line = f32::from(model.cell_size().height);
    let mut typography = Typography::default();
    typography.set_cell(model.cell_size());
    let inner_width = (width - 30.0).max(1.0);
    let mut members = question.member_label();
    if typography.advance(&members, TextRole::Label) > inner_width * 0.4 {
        members = format!("{} steps", question.members.len());
    }
    let title_width = (inner_width - typography.advance(&members, TextRole::Label) - 8.0).max(1.0);
    let mut body = Node::column().min_height(Length::Px(0.0)).child(
        Node::row()
            .child(
                style
                    .label(
                        FlightStyle::fit_label(question.question, title_width, &typography),
                        ink,
                    )
                    .grow(1.0),
            )
            .child(FlightStyle::hspace(8.0))
            .child(style.label(members, style.muted)),
    );
    if height >= line * 3.0 + 36.0 {
        body = body
            .child(FlightStyle::vspace(6.0))
            .child(style.heading(FlightStyle::fit_label(
                question.outcome.unwrap_or("No result recorded yet"),
                inner_width / 1.04,
                &typography,
            )))
            .child(FlightStyle::vspace(6.0))
            .child(style.label(
                FlightStyle::fit_label(&question.relationship(), inner_width, &typography),
                style.muted,
            ));
    } else if height >= line * 2.0 + 30.0 {
        body = body
            .child(FlightStyle::vspace(6.0))
            .child(style.label(question.status_label(), style.muted));
    }
    let border = if selected.is_some_and(|number| question.contains(number)) {
        ink
    } else {
        style.raised
    };
    let node = style
        .outlined(
            style.ground(style.inset(body, 14.0, 11.0), style.raised),
            style.raised,
            border,
        )
        .viewport();
    match question.indices.first() {
        Some(index) => node.id(Widget::trace_row(*index)),
        None => node,
    }
}

fn visible_branches<'a>(model: &'a Model, indices: &[usize]) -> Vec<&'a str> {
    let mut branches = Vec::new();
    for &index in indices {
        if let Some(id) = model.trace().steps()[index].branch_id.as_deref()
            && !branches.contains(&id)
        {
            branches.push(id);
        }
    }
    branches
}

fn selected_number(model: &Model) -> Option<u32> {
    match model.selected() {
        Some(Selected::Step(number)) => Some(*number),
        _ => model
            .trace()
            .selected()
            .or_else(|| model.trace().steps().last())
            .map(|step| step.number),
    }
}

fn rgb(color: Color) -> (u8, u8, u8) {
    match color {
        Color::Rgb(r, g, b) => (r, g, b),
        _ => (30, 30, 46),
    }
}

#[cfg(test)]
#[path = "flight_recorder.tests.rs"]
mod tests;
