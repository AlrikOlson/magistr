//! Native roadmap allocations; the frame reports these bounds to shared input.
use magistr_cockpit::cockpit::{Model, flight_divider::FlightDivider as D};

pub struct RoadmapLayout {
    pub margin: f32,
    pub body: f32,
    pub composer: f32,
    pub left: f32,
    pub right: f32,
    pub graph: f32,
    pub list: f32,
    pub compact: bool,
}
impl RoadmapLayout {
    pub const GAP: f32 = 12.0;
    pub const NAV: f32 = 40.0;
    pub const ROW: f32 = 56.0;
    pub const COLUMNS_MIN: (f32, f32) = (360.0, 300.0);
    pub const GRAPH_MIN: (f32, f32) = (120.0, 160.0);
    pub fn of(model: &Model, width: f32, height: f32) -> Self {
        let margin = if width < 780.0 { 12.0 } else { 20.0 };
        let width = (width - 2.0 * margin).max(1.0);
        let composer = crate::comp_composer::ComposerComposition::height(model, width, height);
        let body = (height - margin - Self::NAV - composer).max(0.0);
        let compact = width < 740.0 || body < 300.0;
        let left = if compact {
            width
        } else {
            crate::workspace_layout::split(
                width - Self::GAP,
                model.panel_fraction(D::Columns, 0.57),
                Self::COLUMNS_MIN,
            )
        };
        let right = if compact {
            width
        } else {
            width - Self::GAP - left
        };
        let available = (body - Self::GAP).max(0.0);
        let graph = crate::workspace_layout::split(
            available,
            model.panel_fraction(D::Graph, 0.47),
            Self::GRAPH_MIN,
        );
        Self {
            margin,
            body,
            composer,
            left,
            right,
            graph,
            list: available - graph,
            compact,
        }
    }
}
