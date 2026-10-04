//! The four adjustable boundaries in the terminal workspace.
use super::{FlightStyle as S, Model, Pane, StageTab, Widget};
use crate::layout::{Align, Length, Node};
use crate::widget_id::WidgetId;

/// A boundary, with geometry shared by drawing and input.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FlightDivider {
    Columns,
    Roadmap,
    Reasoning,
    Graph,
}

impl FlightDivider {
    pub const ALL: [Self; 4] = [Self::Columns, Self::Roadmap, Self::Reasoning, Self::Graph];
    pub const fn index(self) -> usize {
        self as usize
    }
    pub const fn widget(self) -> Widget {
        match self {
            Self::Columns => Widget::PanelColumns,
            Self::Roadmap => Widget::PanelRoadmap,
            Self::Reasoning => Widget::PanelReasoning,
            Self::Graph => Widget::PanelGraph,
        }
    }
    pub(crate) fn from_id(id: WidgetId) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|divider| divider.widget().id() == id)
    }
    pub(crate) fn visible(self, model: &Model) -> bool {
        if model.surface().is_native() {
            return model.flight_panels.native[self.index()].is_some();
        }
        if !model.uses_flight_recorder() {
            return false;
        }
        let l = model.flight_layout();
        match self {
            Self::Columns => !l.compact,
            Self::Roadmap => !l.compact && model.tab() != StageTab::Roadmap,
            Self::Reasoning => {
                model.tab() != StageTab::Roadmap
                    && model.reasoning_expanded()
                    && (!l.compact
                        || (model.pane() != Pane::Inspector && model.tab() == StageTab::Turn))
            }
            Self::Graph => {
                model.tab() == StageTab::Roadmap
                    && l.graph_height > 0.0
                    && l.graph_list_height > 0.0
                    && (!l.compact
                        || (!model.roadmap_requests_open() && model.pane() == Pane::Stage))
            }
        }
    }
    /// Origin, usable extent and current first-panel extent, in physical pixels.
    pub(crate) fn geometry(self, model: &Model) -> (f32, f32, f32) {
        if model.surface().is_native() {
            return model.flight_panels.native[self.index()]
                .map_or((0.0, 1.0, 0.0), |p| (p.origin, p.total, p.first));
        }
        let l = model.flight_layout();
        let top = 24.0 + f32::from(model.cell_size().height).max(28.0);
        match self {
            Self::Columns => (l.margin, (l.width - l.column_gap).max(1.0), l.left_width),
            Self::Roadmap => (top, (l.body_height - l.row_gap).max(1.0), l.roadmap_height),
            Self::Reasoning => (
                top,
                (l.body_height - l.row_gap).max(1.0),
                l.conversation_height,
            ),
            Self::Graph => {
                let nav = if l.compact {
                    f32::from(model.cell_size().height) + 12.0
                } else {
                    0.0
                };
                (
                    top + nav,
                    (l.body_height - nav - l.row_gap).max(1.0),
                    l.graph_height,
                )
            }
        }
    }
    pub(crate) fn minimums(self, line: f32) -> (f32, f32) {
        match self {
            Self::Columns => (
                360.0 * (line / 20.0).max(1.0),
                280.0 * (line / 20.0).max(1.0),
            ),
            Self::Roadmap => (160.0, line * 6.0 + 84.0),
            Self::Reasoning => (line * 2.0 + 70.0, line * 4.0 + 70.0),
            Self::Graph => (180.0, line * 6.0 + 116.0),
        }
    }
    pub(crate) const fn label(self) -> &'static str {
        match self {
            Self::Columns => "Columns",
            Self::Roadmap => "Roadmap and details",
            Self::Reasoning => "Conversation and reasoning",
            Self::Graph => "Graph and list",
        }
    }
    pub(crate) fn view(self, model: &Model) -> Node {
        let s = S::for_model(model);
        let active = model.flight_panels.active() == Some(self);
        let hovered = model.hovered() == Some(self.widget().id());
        let ink = if active || hovered { s.accent } else { s.muted };
        let columns = self == Self::Columns;
        let grip = Node::leaf()
            .width(Length::Px(if columns { 2.0 } else { 32.0 }))
            .height(Length::Px(if columns { 32.0 } else { 2.0 }))
            .shrink(0.0)
            .style(s.style(ink).bg(ink));
        let node = if columns {
            Node::column().width(Length::Px(model.flight_layout().column_gap))
        } else {
            Node::row().height(Length::Px(model.flight_layout().row_gap))
        };
        node.id(self.widget().id())
            .shrink(0.0)
            .min_width(Length::Px(0.0))
            .min_height(Length::Px(0.0))
            .align_items(Align::Center)
            .style(
                s.style(s.ink)
                    .bg(if active || hovered { s.raised } else { s.root }),
            )
            .child(S::flex())
            .child(grip)
            .child(S::flex())
    }
}
