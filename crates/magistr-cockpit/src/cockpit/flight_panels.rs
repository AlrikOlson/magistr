//! Session panel proportions and captured resizing; no terminal protocol state.
use super::{Model, flight_divider::FlightDivider as D};
use crate::input::{Button, KeyCode, KeyEvent, Modifiers, MouseEvent, MouseKind};
use crate::widget_id::WidgetId;

/// Preferred proportions survive compact windows and tab changes.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct FlightPanels {
    proportions: [Option<f32>; 4],
    interaction: Option<Interaction>,
}

#[cfg(test)]
#[path = "flight_panels.tests.rs"]
mod tests;

#[derive(Clone, Debug, PartialEq)]
struct Interaction {
    divider: D,
    offset: Option<f32>,
    before: [Option<f32>; 4],
}

impl FlightPanels {
    pub(crate) fn fraction(&self, divider: D, default: f32) -> f32 {
        self.proportions[divider.index()].unwrap_or(default)
    }

    /// Clamp visible geometry, never the saved preference.
    pub(crate) fn split(total: f32, fraction: f32, minimums: (f32, f32)) -> f32 {
        let total = total.max(0.0);
        let scale = (total / (minimums.0 + minimums.1)).min(1.0);
        (total * fraction).clamp(
            minimums.0 * scale,
            (total - minimums.1 * scale).max(minimums.0 * scale),
        )
    }

    pub(crate) fn active(&self) -> Option<D> {
        self.interaction.as_ref().map(|i| i.divider)
    }

    pub(crate) fn dragged(&self) -> Option<D> {
        self.interaction
            .as_ref()
            .filter(|i| i.offset.is_some())
            .map(|i| i.divider)
    }

    pub(crate) fn cancel(&mut self) {
        if let Some(i) = self.interaction.take() {
            self.proportions = i.before;
        }
    }

    pub(crate) fn reset(&mut self) {
        self.proportions = [None; 4];
    }

    pub(crate) fn begin_keyboard(model: &mut Model) {
        if let Some(divider) = D::ALL.into_iter().find(|d| d.visible(model)) {
            model.flight_panels.interaction = Some(Interaction {
                divider,
                offset: None,
                before: model.flight_panels.proportions,
            });
            model.session.completion = None;
        }
    }

    fn resize(model: &mut Model, divider: D, value: f32) {
        if !divider.visible(model) {
            return;
        }
        let (_, total, _) = divider.geometry(model);
        let first = Self::split(
            total,
            value / total,
            divider.minimums(f32::from(model.cell_size().height)),
        );
        model.flight_panels.proportions[divider.index()] = Some(first / total);
    }

    fn coordinate(model: &Model, event: &MouseEvent, divider: D) -> f32 {
        // Exact pixels when supplied; cell centres otherwise, at current font metrics.
        let cell = model.cell_size();
        let (x, y) = event.px.map_or_else(
            || {
                (
                    (f32::from(event.col) + 0.5) * f32::from(cell.width),
                    (f32::from(event.row) + 0.5) * f32::from(cell.height),
                )
            },
            |(x, y)| (x as f32, y as f32),
        );
        if divider == D::Columns { x } else { y }
    }

    /// True means this gesture owns the event, including outside its original rect.
    pub(crate) fn mouse(model: &mut Model, event: MouseEvent, target: Option<WidgetId>) -> bool {
        if let Some(i) = model.flight_panels.interaction.clone() {
            if let Some(offset) = i.offset {
                match event.kind {
                    MouseKind::Drag(Button::Left) | MouseKind::Up(Button::Left) => {
                        let (origin, _, _) = i.divider.geometry(model);
                        Self::resize(
                            model,
                            i.divider,
                            Self::coordinate(model, &event, i.divider) - origin - offset,
                        );
                        if matches!(event.kind, MouseKind::Up(_)) {
                            model.flight_panels.interaction = None;
                            model.pressed = None;
                            model.hovered = target;
                        }
                    }
                    MouseKind::Down(_) | MouseKind::Up(_) => {
                        model.flight_panels.cancel();
                        model.pressed = None;
                    }
                    _ => {}
                }
                return true;
            }
            // A click finishes keyboard adjustment before taking its usual action.
            if matches!(event.kind, MouseKind::Down(_)) {
                model.flight_panels.interaction = None;
            }
        }
        if event.kind != MouseKind::Down(Button::Left)
            || !model.uses_flight_recorder()
            || model.is_sheet_open()
            || model.settings.is_some()
            || model.search.is_some()
            || model.palette.is_some()
            || model.moment().is_some()
        {
            return false;
        }
        let Some(divider) = target.and_then(D::from_id).filter(|d| d.visible(model)) else {
            return false;
        };
        let (origin, _, first) = divider.geometry(model);
        model.flight_panels.interaction = Some(Interaction {
            divider,
            offset: Some(Self::coordinate(model, &event, divider) - origin - first),
            before: model.flight_panels.proportions,
        });
        model.roadmap_drag = None;
        model.pressed = Some(divider.widget().id());
        model.hovered = model.pressed;
        true
    }

    pub(crate) fn key(model: &mut Model, key: KeyEvent) -> bool {
        let Some(i) = model.flight_panels.interaction.clone() else {
            return false;
        };
        match key.code {
            KeyCode::Esc => {
                model.flight_panels.cancel();
                model.pressed = None;
            }
            KeyCode::Enter if i.offset.is_none() => {
                model.flight_panels.interaction = None;
            }
            KeyCode::Tab if i.offset.is_none() => {
                let visible: Vec<_> = D::ALL.into_iter().filter(|d| d.visible(model)).collect();
                if !visible.is_empty() {
                    let current = visible.iter().position(|d| *d == i.divider).unwrap_or(0);
                    let next = if key.mods.contains(Modifiers::SHIFT) {
                        (current + visible.len() - 1) % visible.len()
                    } else {
                        (current + 1) % visible.len()
                    };
                    if let Some(interaction) = model.flight_panels.interaction.as_mut() {
                        interaction.divider = visible[next];
                    }
                }
            }
            KeyCode::Char('r') if i.offset.is_none() => model.flight_panels.reset(),
            KeyCode::Left | KeyCode::Right | KeyCode::Up | KeyCode::Down if i.offset.is_none() => {
                let columns = i.divider == D::Columns;
                let along = if columns {
                    matches!(key.code, KeyCode::Left | KeyCode::Right)
                } else {
                    matches!(key.code, KeyCode::Up | KeyCode::Down)
                };
                if along {
                    let cell = model.cell_size();
                    let step = if columns {
                        f32::from(cell.width) * 2.0
                    } else {
                        f32::from(cell.height)
                    };
                    let direction = if matches!(key.code, KeyCode::Left | KeyCode::Up) {
                        -1.0
                    } else {
                        1.0
                    };
                    let (_, _, first) = i.divider.geometry(model);
                    Self::resize(model, i.divider, first + step * direction);
                }
            }
            KeyCode::Home | KeyCode::End if i.offset.is_none() => {
                let (_, total, _) = i.divider.geometry(model);
                Self::resize(
                    model,
                    i.divider,
                    if key.code == KeyCode::Home {
                        0.0
                    } else {
                        total
                    },
                );
            }
            _ => {}
        }
        true
    }
}
