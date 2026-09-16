//! The live roadmap neighborhood, with shared pixel positions and meaningful labels.

use super::{DagNode, FlightStyle, Model, Motion, Widget};
use crate::layout::{Align, Inset, Length, Node, TextRole};
use crate::pixel::{RoadmapDag, Typography};
use crate::render::Color;
use unicode_segmentation::UnicodeSegmentation;

/// The upper-right roadmap panel in the Flight Recorder workspace.
pub struct FlightRoadmap;

impl FlightRoadmap {
    #[must_use]
    pub fn view(model: &Model, width: f32, height: f32) -> Node {
        let s = FlightStyle::for_model(model);
        let graph_width = (width - 40.0).max(1.0);
        let graph_height = Self::graph_height(model, height);
        let (nodes, _) = model.roadmap().mini_pixels(graph_width, graph_height);
        let mut body = Node::column()
            .min_height(Length::Px(0.0))
            .child(FlightStyle::vspace(5.0));
        if nodes.is_empty() {
            body = body.child(s.prose("No work items yet. Open Roadmap to see your goals."));
        } else {
            let mut graph = Node::column()
                .height(Length::Px(graph_height))
                .shrink(0.0)
                .child(
                    Node::leaf()
                        .height(Length::Px(graph_height))
                        .id(Widget::RailGraph.id()),
                );
            for (index, node) in &nodes {
                graph = graph.child(
                    Node::leaf()
                        .width(Length::Px(24.0))
                        .height(Length::Px(24.0))
                        .overlay(
                            Inset::none()
                                .left(Length::Px(node.x - 12.0))
                                .top(Length::Px(node.y - 12.0)),
                        )
                        .id(Widget::sky_star(*index)),
                );
            }
            body = body
                .child(graph)
                .child(FlightStyle::vspace(4.0))
                .child(Self::labels(model, &s, &nodes, graph_width));
        }
        s.outlined(
            Node::column()
                .min_height(Length::Px(0.0))
                .child(s.section_heading(
                    "Roadmap",
                    s.label("Open →", s.accent).id(Widget::TabRoadmap.id()),
                ))
                .child(s.inset(body, 19.0, 8.0).grow(1.0).viewport()),
            s.panel,
            s.line,
        )
        .height(Length::Px(height))
        .shrink(0.0)
    }

    #[must_use]
    pub fn graph_height(model: &Model, panel_height: f32) -> f32 {
        let line = f32::from(model.cell_size().height);
        (panel_height - (line + 14.0).max(34.0) - line - 27.0).clamp(8.0, 60.0)
    }

    #[must_use]
    pub fn nodes(model: &Model, width: f32, height: f32) -> Vec<(usize, DagNode)> {
        model.roadmap().mini_pixels(width, height).0
    }

    #[must_use]
    pub fn graph(model: &Model, width: f32, height: f32) -> RoadmapDag {
        let (nodes, edges) = model.roadmap().mini_pixels(width, height);
        let s = FlightStyle::for_model(model);
        let (r, g, b) = match s.panel {
            Color::Rgb(r, g, b) => (r, g, b),
            _ => (30, 30, 46),
        };
        let phase = if model.theme().motion().is_full() {
            Motion::glow_phase(model.now())
        } else {
            0.0
        };
        RoadmapDag::pixels(
            nodes.into_iter().map(|(_, node)| node).collect::<Vec<_>>(),
            edges,
            20.0,
        )
        .bands(false)
        .light(model.theme().mode() == crate::theme::ThemeMode::Light)
        .background(r, g, b)
        .glow(phase)
    }

    fn labels(model: &Model, s: &FlightStyle, nodes: &[(usize, DagNode)], width: f32) -> Node {
        let mut ranks: Vec<(f32, Vec<usize>)> = Vec::new();
        for (index, node) in nodes {
            if let Some((_, members)) = ranks.iter_mut().find(|(x, _)| (*x - node.x).abs() < 0.1) {
                members.push(*index);
            } else {
                ranks.push((node.x, vec![*index]));
            }
        }
        ranks.sort_by(|a, b| a.0.total_cmp(&b.0));
        let mut labels = Node::row().height(Length::Px(s.line_height)).shrink(0.0);
        let mut typography = Typography::default();
        typography.set_cell(model.cell_size());
        for (at, (x, members)) in ranks.iter().enumerate() {
            let chunks: Vec<_> = members
                .iter()
                .filter_map(|&index| model.roadmap().graph_chunk(index))
                .collect();
            let first = chunks[0];
            let title = if chunks.len() > 1
                && first.group.is_some()
                && chunks.iter().all(|chunk| chunk.group == first.group)
            {
                first.group.as_deref().unwrap_or("Work items").to_owned()
            } else if chunks.len() == 1 {
                first.name.clone()
            } else {
                format!("{} +{}", first.name, chunks.len() - 1)
            };
            let left = if at == 0 {
                0.0
            } else {
                (ranks[at - 1].0 + x) / 2.0
            };
            let right = if at + 1 == ranks.len() {
                width
            } else {
                (x + ranks[at + 1].0) / 2.0
            };
            let available = (right - left - 8.0).max(1.0);
            let title = fit(&title, available, &typography);
            let ink = if chunks.iter().any(|chunk| chunk.is_in_progress()) {
                s.accent
            } else if chunks.iter().all(|chunk| chunk.status == "done") {
                s.good
            } else {
                s.muted
            };
            let alignment = if at == 0 {
                Align::Start
            } else if at + 1 == ranks.len() {
                Align::End
            } else {
                Align::Center
            };
            labels = labels.child(
                Node::column()
                    .align_items(alignment)
                    .child(s.label(title, ink))
                    .width(Length::Px((right - left).max(1.0)))
                    .height(Length::Px(s.line_height))
                    .overlay(Inset::none().left(Length::Px(left)).top(Length::Px(0.0)))
                    .id(Widget::sky_star(members[0])),
            );
        }
        labels
    }
}

fn fit(text: &str, width: f32, typography: &Typography) -> String {
    if typography.advance(text, TextRole::Label) <= width {
        return text.to_owned();
    }
    let mut result = String::new();
    let mut advance = typography.advance("…", TextRole::Label);
    for grapheme in text.graphemes(true) {
        advance += typography.advance(grapheme, TextRole::Label);
        if advance > width {
            break;
        }
        result.push_str(grapheme);
    }
    result.push('…');
    result
}

#[cfg(test)]
#[path = "flight_roadmap.tests.rs"]
mod tests;
