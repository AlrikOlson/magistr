//! Dedicated roadmap browsing and requests, using live selection and draft state.
use super::flight_divider::FlightDivider as D;
use super::inspector::Inspector;
use super::{
    FlightRecords, FlightRequests, FlightRoadmapGraph, FlightStyle as S, Model, Pane, Widget,
};
use crate::layout::{Length, Node};

pub struct FlightRoadmapPage;

impl FlightRoadmapPage {
    #[must_use]
    pub fn graph_height(model: &Model) -> f32 {
        model.flight_layout().graph_height
    }

    #[must_use]
    pub fn list_height(model: &Model) -> f32 {
        model.flight_layout().graph_list_height
    }

    #[must_use]
    pub fn detail_height(model: &Model) -> f32 {
        (model.flight_layout().body_height - Self::navigation_height(model)).max(0.0)
    }

    fn navigation_height(model: &Model) -> f32 {
        f32::from(model.cell_size().height) + 12.0
    }

    fn navigation(model: &Model) -> Node {
        let s = S::for_model(model);
        let requests = model.roadmap_requests_open();
        let tab = |label, widget: Widget, active| {
            s.chip(
                label,
                if active { s.ink } else { s.accent },
                if active || model.hovered() == Some(widget.id()) {
                    s.selected
                } else {
                    s.root
                },
            )
            .id(widget.id())
        };
        let mut row = Node::row()
            .height(Length::Px(Self::navigation_height(model)))
            .shrink(0.0);
        if model.flight_layout().compact {
            row = row
                .child(tab(
                    "Back to roadmap",
                    Widget::TabRoadmap,
                    !requests && model.pane() == Pane::Stage,
                ))
                .child(S::hspace(8.0));
        }
        row.child(tab("Requests", Widget::RoadmapRequests, requests))
            .child(S::hspace(8.0))
            .child(tab(
                "Item details",
                Widget::RoadmapDetails,
                !requests && (!model.flight_layout().compact || model.pane() != Pane::Stage),
            ))
    }

    fn detail(model: &Model, width: f32, height: f32) -> Node {
        if model.roadmap_requests_open() {
            FlightRequests::view(model, width, height)
        } else {
            Inspector::flight_view(model, width, height)
        }
    }

    #[must_use]
    pub fn view(model: &Model) -> Node {
        let l = model.flight_layout();
        let detail_height = Self::detail_height(model);
        if l.compact && (model.roadmap_requests_open() || model.pane() != Pane::Stage) {
            return Node::column()
                .height(Length::Px(l.body_height))
                .child(Self::navigation(model))
                .child(
                    Self::detail(model, l.width, detail_height)
                        .width(Length::Px(l.width))
                        .height(Length::Px(detail_height)),
                );
        }
        let graph_height = Self::graph_height(model);
        let mut plan = Node::column()
            .width(Length::Px(l.left_width))
            .min_height(Length::Px(0.0))
            .shrink(0.0);
        if l.compact {
            plan = plan.child(Self::navigation(model));
        }
        if graph_height > 0.0 {
            plan = plan.child(FlightRoadmapGraph::view(model, l.left_width, graph_height));
            if l.graph_list_height > 0.0 {
                plan = plan.child(D::Graph.view(model));
            }
        }
        if l.graph_list_height > 0.0 {
            plan = plan.child(
                FlightRecords::view(model, l.left_width, Self::list_height(model))
                    .height(Length::Px(Self::list_height(model)))
                    .shrink(0.0),
            );
        }
        let mut body = Node::row()
            .height(Length::Px(l.body_height))
            .min_height(Length::Px(0.0))
            .shrink(0.0)
            .child(plan);
        if !l.compact {
            body = body.child(D::Columns.view(model)).child(
                Node::column()
                    .width(Length::Px(l.right_width))
                    .height(Length::Px(l.body_height))
                    .shrink(0.0)
                    .min_height(Length::Px(0.0))
                    .child(Self::navigation(model))
                    .child(
                        Self::detail(model, l.right_width, detail_height)
                            .width(Length::Px(l.right_width))
                            .height(Length::Px(detail_height))
                            .shrink(0.0),
                    ),
            );
        }
        body
    }
}
