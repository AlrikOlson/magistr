//! Pixel allocations shared by the production view and its model viewports.

use super::{
    FlightComposer, FlightRecorder, FlightRecords, Model, flight_divider::FlightDivider as D,
    flight_panels::FlightPanels,
};

/// The same physical rectangles govern composition, wrapping and scrolling.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FlightLayout {
    pub screen_width: f32,
    pub screen_height: f32,
    pub margin: f32,
    pub width: f32,
    pub left_width: f32,
    pub right_width: f32,
    pub body_height: f32,
    pub column_gap: f32,
    pub row_gap: f32,
    pub conversation_height: f32,
    pub recorder_height: f32,
    pub roadmap_height: f32,
    pub detail_height: f32,
    pub graph_height: f32,
    pub graph_list_height: f32,
    pub composer_height: f32,
    pub extra_height: f32,
    pub compact: bool,
}

impl FlightLayout {
    #[must_use]
    pub fn of(model: &Model) -> Self {
        let cell = model.cell_size();
        let screen_width = f32::from(model.size().cols) * f32::from(cell.width);
        let screen_height = f32::from(model.size().rows) * f32::from(cell.height);
        let margin = if screen_width < 600.0 { 8.0 } else { 20.0 };
        let width = (screen_width - margin * 2.0).max(1.0);
        let composer_height = FlightComposer::height(model, width, screen_height);
        let line = f32::from(cell.height);
        let extra_height =
            super::FlightDock::height(model, width) + super::FlightDock::terminal_height(model);
        let header = line.max(28.0);
        let body_height =
            (screen_height - 24.0 - header - 24.0 - composer_height - extra_height).max(0.0);
        let panels = &model.flight_panels;
        // Cell-only mouse reports address cell centres: every grab area spans a cell.
        let column_gap = f32::from(cell.width).max(20.0);
        let row_gap = line.max(16.0);
        let column_min = D::Columns.minimums(line);
        let compact = width < column_min.0 + column_min.1 + column_gap
            || body_height < row_gap + 160.0 + line * 6.0 + 84.0;
        let columns = (width - column_gap).max(0.0);
        let default_right = if screen_width >= 1280.0 {
            400.0 + (screen_width - 1280.0) * 0.25
        } else {
            (width * 0.33).clamp(250.0, 400.0)
        };
        let left_width = if compact {
            width
        } else {
            FlightPanels::split(
                columns,
                panels.fraction(D::Columns, 1.0 - default_right / columns.max(1.0)),
                D::Columns.minimums(line),
            )
        };
        let right_width = if compact { 0.0 } else { columns - left_width };
        let stack = (body_height - row_gap).max(0.0);
        let conversation_stack = (body_height
            - if model.reasoning_expanded() {
                row_gap
            } else {
                16.0
            })
        .max(0.0);
        let desired = (FlightRecorder::height(model, left_width).min(body_height * 0.48)
            - if model.reasoning_expanded() {
                row_gap - 16.0
            } else {
                0.0
            })
        .max(0.0);
        let conversation_height = if model.reasoning_expanded() {
            FlightPanels::split(
                stack,
                panels.fraction(D::Reasoning, 1.0 - desired / stack.max(1.0)),
                D::Reasoning.minimums(line),
            )
        } else {
            (conversation_stack - desired).max(0.0)
        };
        let recorder_height = (conversation_stack - conversation_height).max(0.0);
        let roadmap_height = FlightPanels::split(
            stack,
            panels.fraction(D::Roadmap, 0.48),
            D::Roadmap.minimums(line),
        );
        let detail_height = (stack - roadmap_height).max(0.0);
        let nav = if compact { line + 12.0 } else { 0.0 };
        let content = (body_height - nav).max(0.0);
        let graph_stack = (content - row_gap).max(0.0);
        let track_min = (line + 14.0).max(34.0)
            + 22.0
            + (line + 4.0) * if compact { 2.0 } else { 3.0 }
            + 12.0
            + FlightRecords::track_row_height(model) * 2.0;
        let (graph_height, graph_list_height) = if !model.roadmap().is_dependencies_view() {
            (0.0, content)
        } else if graph_stack >= D::Graph.minimums(line).0 + track_min {
            let graph = FlightPanels::split(
                graph_stack,
                panels.fraction(D::Graph, 0.55),
                (D::Graph.minimums(line).0, track_min),
            );
            (graph, graph_stack - graph)
        } else {
            (content, 0.0)
        };
        Self {
            screen_width,
            screen_height,
            margin,
            width,
            left_width,
            right_width,
            body_height,
            column_gap,
            row_gap,
            conversation_height,
            recorder_height,
            roadmap_height,
            detail_height,
            graph_height,
            graph_list_height,
            composer_height,
            extra_height,
            compact,
        }
    }

    #[must_use]
    pub fn conversation_rows(self, model: &Model) -> usize {
        let heading = if self.conversation_height >= 380.0 {
            94.0
        } else {
            70.0
        };
        ((self.conversation_height - heading).max(0.0) / f32::from(model.cell_size().height))
            .floor() as usize
    }
}
