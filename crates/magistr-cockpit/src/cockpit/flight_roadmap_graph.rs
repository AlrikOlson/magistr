//! The full roadmap dependency graph, sharing physical geometry with pointer input.

use super::{DagNode, FlightStyle, Model, Motion, Roadmap, Widget};
use crate::layout::{Inset, Length, Node, PxRect};
use crate::pixel::{RoadmapDag, Typography};
use crate::render::Color;

/// The dedicated roadmap page's dependency graph and named selection targets.
pub struct FlightRoadmapGraph;

impl FlightRoadmapGraph {
    #[must_use]
    pub fn view(model: &Model, width: f32, height: f32) -> Node {
        let s = FlightStyle::for_model(model);
        let heading = (s.line_height + 14.0).max(34.0);
        let graph_width = (width - 2.0).max(1.0);
        let graph_height = (height - heading - s.line_height - 18.0).max(1.0);
        let roadmap = Self::project(model, graph_width, graph_height);
        let nodes = roadmap.dag_nodes_at(model.now());
        let mut graph = Node::column()
            .height(Length::Px(graph_height))
            .shrink(0.0)
            .viewport()
            .id(Widget::RoadmapGraph.id());
        if nodes.is_empty() {
            graph = graph.child(s.inset(
                s.prose("Work items and their dependencies appear here."),
                18.0,
                16.0,
            ));
        }
        for (index, node) in nodes.iter().enumerate() {
            if node.x < 0.0 || node.x > graph_width || node.y < 0.0 || node.y > graph_height {
                continue;
            }
            let radius = nodes
                .iter()
                .enumerate()
                .filter(|(other, _)| *other != index)
                .map(|(_, other)| (node.x - other.x).hypot(node.y - other.y) / 2.0)
                .fold(12.0_f32, f32::min)
                .max(0.5);
            let left = (node.x - radius).max(0.0);
            let top = (node.y - radius).max(0.0);
            let right = (node.x + radius).min(graph_width);
            let bottom = (node.y + radius).min(graph_height);
            graph = graph.child(
                Node::leaf()
                    .width(Length::Px(right - left))
                    .height(Length::Px(bottom - top))
                    .overlay(Inset::none().left(Length::Px(left)).top(Length::Px(top)))
                    .id(Widget::sky_star(index)),
            );
        }
        let mut typography = Typography::default();
        typography.set_cell(model.cell_size());
        let mut labels = Vec::<PxRect>::new();
        let mut order: Vec<_> = (0..nodes.len()).collect();
        order.sort_by_key(|&index| {
            (
                model.hovered() != Some(Widget::sky_star(index)),
                !nodes[index].selected,
            )
        });
        for index in order {
            let node = &nodes[index];
            if node.x < 0.0 || node.x > graph_width || node.y < 0.0 || node.y > graph_height {
                continue;
            }
            let Some(chunk) = roadmap.graph_chunk(index) else {
                continue;
            };
            let available = (s.line_height * 6.0 + 12.0).min(graph_width);
            let title = FlightStyle::fit_label(&chunk.name, available, &typography);
            let label_width = typography.advance(&title, crate::layout::TextRole::Label) + 8.0;
            let x = (node.x - label_width / 2.0).clamp(0.0, (graph_width - label_width).max(0.0));
            let below = node.y + 13.0;
            let y = if below + s.line_height + 4.0 <= graph_height {
                below
            } else {
                (node.y - s.line_height - 17.0).max(0.0)
            };
            let rect = PxRect::new(x, y, label_width, s.line_height + 4.0);
            if y + rect.height > graph_height
                || nodes.iter().enumerate().any(|(other, point)| {
                    other != index
                        && point.x + 3.0 > rect.x
                        && point.x - 3.0 < rect.x + rect.width
                        && point.y + 3.0 > rect.y
                        && point.y - 3.0 < rect.y + rect.height
                })
                || labels.iter().any(|prior| {
                    rect.x < prior.x + prior.width + 6.0
                        && rect.x + rect.width + 6.0 > prior.x
                        && rect.y < prior.y + prior.height + 2.0
                        && rect.y + rect.height + 2.0 > prior.y
                })
            {
                continue;
            }
            labels.push(rect);
            let color = if node.selected || node.here {
                s.accent
            } else {
                s.ink
            };
            let background = if node.selected {
                s.selected
            } else if model.hovered() == Some(Widget::sky_star(index)) {
                s.raised
            } else {
                s.panel
            };
            graph = graph.child(
                s.ground(s.inset(s.label(title, color), 4.0, 2.0), background)
                    .width(Length::Px(rect.width))
                    .height(Length::Px(rect.height))
                    .overlay(
                        Inset::none()
                            .left(Length::Px(rect.x))
                            .top(Length::Px(rect.y)),
                    )
                    .id(Widget::sky_star(index)),
            );
        }
        let scale = format!("{:.0}%", roadmap.zoom() * 100.0);
        let named = model
            .hovered()
            .and_then(Widget::sky_star_index)
            .and_then(|index| roadmap.graph_chunk(index))
            .or_else(|| roadmap.selected());
        let scale = named.map_or_else(
            || scale.clone(),
            |chunk| {
                let name =
                    FlightStyle::fit_label(&chunk.name, (width - 360.0).max(40.0), &typography);
                format!("{name} · {scale}")
            },
        );
        s.outlined(
            Node::column()
                .child(
                    s.section_heading(
                        "Dependencies",
                        Node::row()
                            .align_items(crate::layout::Align::Center)
                            .child(s.label(scale, s.muted))
                            .child(FlightStyle::hspace(12.0))
                            .child(
                                s.chip("Reset view", s.accent, s.raised)
                                    .id(Widget::RoadmapReset.id()),
                            ),
                    ),
                )
                .child(graph)
                .child(s.inset(s.label("Drag to move · Scroll to zoom", s.muted), 16.0, 7.0)),
            s.panel,
            s.line,
        )
        .height(Length::Px(height))
        .shrink(0.0)
    }

    /// Shared point projection for rendering, labels, and physical hit targets.
    #[must_use]
    pub fn nodes(model: &Model, width: f32, height: f32) -> Vec<DagNode> {
        Self::project(model, width, height).dag_nodes_at(model.now())
    }

    /// Native paths use exactly the point projection the view places.
    #[must_use]
    pub fn graph(model: &Model, width: f32, height: f32) -> RoadmapDag {
        let roadmap = Self::project(model, width, height);
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
        let (ar, ag, ab) = match s.accent {
            Color::Rgb(r, g, b) => (r, g, b),
            _ => (151, 191, 255),
        };
        RoadmapDag::pixels(
            roadmap.dag_nodes_at(model.now()),
            roadmap.dag_edges(),
            s.line_height,
        )
        .routes(roadmap.dag_paths_at(model.now()))
        .highlight_incident(ar, ag, ab)
        .bands(false)
        .light(model.theme().mode() == crate::theme::ThemeMode::Light)
        .background(r, g, b)
        .glow(phase)
    }

    fn project(model: &Model, width: f32, height: f32) -> Roadmap {
        let mut roadmap = model.roadmap().clone();
        roadmap.set_pixel_viewport(Some((width, height)), f32::from(model.cell_size().height));
        roadmap
    }
}

#[cfg(test)]
#[path = "flight_roadmap_graph.tests.rs"]
mod tests;
