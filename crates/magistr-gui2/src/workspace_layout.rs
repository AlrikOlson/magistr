//! Logical desktop panel allocations. Painting and input read the same placed boxes.
use magistr_cockpit::cockpit::{Model, flight_divider::FlightDivider as D};

pub struct WorkspaceLayout {
    pub body: f32,
    pub status: f32,
    pub composer: f32,
    pub margin: f32,
    pub left: f32,
    pub right: f32,
    pub conversation: f32,
    pub reasoning: f32,
    pub nearby: f32,
    pub details: f32,
    pub compact: bool,
}

impl WorkspaceLayout {
    pub const GAP: f32 = 12.0;
    pub const COLUMNS_MIN: (f32, f32) = (360.0, 280.0);
    pub const REASONING_MIN: (f32, f32) = (110.0, 112.0);
    pub const DETAILS_MIN: (f32, f32) = (116.0, 170.0);

    pub fn of(model: &Model, width: f32, height: f32) -> Self {
        let margin = if width < 720.0 { 12.0 } else { 20.0 };
        let width = (width - margin * 2.0).max(1.0);
        let status = if width >= 860.0 { 32.0 } else { 0.0 };
        let composer = crate::comp_composer::ComposerComposition::height(model, width, height);
        let body = (height - margin - composer - status).max(0.0);
        let compact = width < 700.0 || body < 300.0;
        let left = if compact {
            width
        } else {
            split(
                width - Self::GAP,
                model.panel_fraction(D::Columns, 0.65),
                Self::COLUMNS_MIN,
            )
        };
        let right = if compact {
            width
        } else {
            width - Self::GAP - left
        };
        let left_height = if compact {
            (body * 0.62 - Self::GAP).max(0.0)
        } else {
            body
        };
        let (conversation, reasoning) = if model.reasoning_expanded() {
            let stack = (left_height - Self::GAP).max(0.0);
            let conversation = split(
                stack,
                model.panel_fraction(D::Reasoning, 0.62),
                Self::REASONING_MIN,
            );
            (conversation, stack - conversation)
        } else {
            ((left_height - 36.0).max(0.0), left_height.min(36.0))
        };
        let (nearby, details) = if compact {
            (0.0, (body - left_height - Self::GAP).max(0.0))
        } else {
            let stack = (body - Self::GAP).max(0.0);
            let nearby = split(
                stack,
                model.panel_fraction(D::Roadmap, 0.35),
                Self::DETAILS_MIN,
            );
            (nearby, stack - nearby)
        };
        Self {
            body,
            status,
            composer,
            margin,
            left,
            right,
            conversation,
            reasoning,
            nearby,
            details,
            compact,
        }
    }
}

pub(crate) fn split(total: f32, fraction: f32, minimums: (f32, f32)) -> f32 {
    let total = total.max(0.0);
    let scale = (total / (minimums.0 + minimums.1)).min(1.0);
    (total * fraction).clamp(minimums.0 * scale, (total - minimums.1 * scale).max(0.0))
}
