//! Dedicated roadmap browsing, using the same live selection and draft.
use super::inspector::Inspector;
use super::{FlightRecords, FlightRoadmapGraph, FlightStyle as S, Model, Pane};
use crate::layout::{Length, Node};

pub struct FlightRoadmapPage;

impl FlightRoadmapPage {
    #[must_use]
    pub fn graph_height(model: &Model) -> f32 {
        let l = model.flight_layout();
        let minimum_list = f32::from(model.cell_size().height) * 6.0 + 116.0;
        if l.body_height < minimum_list + 180.0 || l.left_width < 480.0 {
            0.0
        } else {
            (l.body_height * 0.35).min(280.0)
        }
    }

    #[must_use]
    pub fn list_height(model: &Model) -> f32 {
        let graph = Self::graph_height(model);
        (model.flight_layout().body_height - graph - if graph > 0.0 { 16.0 } else { 0.0 }).max(0.0)
    }

    #[must_use]
    pub fn view(model: &Model) -> Node {
        let l = model.flight_layout();
        if l.compact && model.pane() != Pane::Stage {
            return Inspector::flight_view(model, l.width, l.body_height)
                .width(Length::Px(l.width))
                .height(Length::Px(l.body_height));
        }
        let graph_height = Self::graph_height(model);
        let mut plan = Node::column()
            .width(Length::Px(l.left_width))
            .min_height(Length::Px(0.0))
            .shrink(0.0);
        if graph_height > 0.0 {
            plan = plan
                .child(FlightRoadmapGraph::view(model, l.left_width, graph_height))
                .child(S::vspace(16.0));
        }
        plan = plan.child(
            FlightRecords::view(model, l.left_width, Self::list_height(model))
                .height(Length::Px(Self::list_height(model)))
                .shrink(0.0),
        );
        let mut body = Node::row()
            .height(Length::Px(l.body_height))
            .min_height(Length::Px(0.0))
            .shrink(0.0)
            .child(plan);
        if !l.compact {
            body = body.child(S::hspace(20.0)).child(
                Inspector::flight_view(model, l.right_width, l.body_height)
                    .width(Length::Px(l.right_width))
                    .height(Length::Px(l.body_height))
                    .shrink(0.0),
            );
        }
        body
    }
}
