//! Pixel allocations shared by the production view and its model viewports.

use super::{FlightComposer, FlightRecorder, Model};

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
    pub conversation_height: f32,
    pub recorder_height: f32,
    pub roadmap_height: f32,
    pub detail_height: f32,
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
        // The right list needs its controls and at least one complete work item.
        let compact = width < 660.0 || body_height * 0.68 - 16.0 < line * 6.0 + 84.0;
        let right_width = if compact {
            0.0
        } else if screen_width >= 1280.0 {
            (400.0 + (screen_width - 1280.0) * 0.25).min(500.0)
        } else {
            (width * 0.33).clamp(250.0, 400.0)
        };
        let left_width = (width - right_width - if compact { 0.0 } else { 20.0 }).max(1.0);

        let desired = FlightRecorder::height(model, left_width);
        let recorder_height = desired.min((body_height * 0.48).max(0.0));
        let conversation_height = (body_height - recorder_height - 16.0).max(0.0);
        let roadmap_height = 148.0_f32.min(body_height * 0.32);
        let detail_height = (body_height - roadmap_height - 16.0).max(0.0);
        Self {
            screen_width,
            screen_height,
            margin,
            width,
            left_width,
            right_width,
            body_height,
            conversation_height,
            recorder_height,
            roadmap_height,
            detail_height,
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
