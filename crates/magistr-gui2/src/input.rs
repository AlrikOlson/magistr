use magistr_cockpit::cockpit::{Msg, Size};
use magistr_cockpit::hit::HitRegistry;
use magistr_cockpit::input::{
    Button, KeyCode, KeyEvent, KeyKind, Modifiers, MouseEvent, MouseKind,
};
use winit::event::{ElementState, MouseButton, MouseScrollDelta, WindowEvent};
use winit::keyboard::{Key, ModifiersState, NamedKey};

use crate::metrics::CellMetrics;

/// winit's input, as the cockpit's own [`Msg`].
///
/// The only translation layer the native GUI has, and it is deliberately
/// thin: a pointer position becomes a cell through
/// [`CellMetrics::cell_at`] and the *terminal's* [`HitRegistry`] answers
/// for it, so there is no second hit path to disagree with the first.
/// Everything past this module is the same fold the terminal runs.
///
/// # What is carried and what is not
///
/// A pressed button is remembered, because winit reports a drag as a
/// bare `CursorMoved` while a terminal reports `Drag(button)` — a
/// cockpit that could not tell a drag from a hover would lose the rail
/// divider and the tech-tree's pan. A scroll is quantized to notches the
/// same way a terminal's wheel reports are, so one gesture is one
/// `ScrollUp` however the trackpad phrases it.
#[derive(Clone, Copy, Debug, Default)]
pub struct Input {
    /// Where the pointer last was, in physical pixels.
    at: (f64, f64),
    /// The button held, for telling a drag from a hover.
    held: Option<Button>,
    /// Modifier state, which winit reports as its own event rather than
    /// on each key the way a terminal does.
    mods: Modifiers,
    /// Accumulated pixel scroll, for turning a trackpad's continuous
    /// delta into the discrete notches the cockpit's viewports expect.
    scrolled: f64,
}

impl Input {
    /// Pixels of continuous scroll that make one notch. A line delta is
    /// one notch outright; a pixel delta is not, and picking the cell
    /// height would make a notch mean "one row" on a coarse grid and
    /// "one row" on a fine one — which is the same thing said twice, so
    /// it is the cell height.
    fn notch(metrics: &CellMetrics) -> f64 {
        f64::from(metrics.height())
    }

    /// Fold `event` into whatever messages the cockpit should see.
    ///
    /// Several rather than one: a scroll of three notches is three
    /// `Msg::Mouse`es, exactly as a terminal sends three wheel reports,
    /// so the cockpit's own coalescing (`Viewport::scroll_by` folds them
    /// into one glide) is what decides how it looks.
    pub fn fold(
        &mut self,
        event: &WindowEvent,
        metrics: &CellMetrics,
        size: Size,
        hits: &HitRegistry,
        out: &mut Vec<Msg>,
    ) {
        match event {
            WindowEvent::ModifiersChanged(mods) => self.mods = modifiers(mods.state()),
            WindowEvent::Focused(focused) => out.push(Msg::Focus(*focused)),
            WindowEvent::CursorMoved { position, .. } => {
                self.at = (position.x, position.y);
                let kind = match self.held {
                    Some(button) => MouseKind::Drag(button),
                    None => MouseKind::Move,
                };
                self.mouse(kind, metrics, size, hits, out);
            }
            WindowEvent::CursorLeft { .. } => self.held = None,
            WindowEvent::MouseInput { state, button, .. } => {
                let Some(button) = self::button(*button) else {
                    return;
                };
                let kind = match state {
                    ElementState::Pressed => {
                        self.held = Some(button);
                        MouseKind::Down(button)
                    }
                    ElementState::Released => {
                        self.held = None;
                        MouseKind::Up(button)
                    }
                };
                self.mouse(kind, metrics, size, hits, out);
            }
            WindowEvent::MouseWheel { delta, .. } => {
                let (dx, dy) = match delta {
                    // A line delta is already notches.
                    MouseScrollDelta::LineDelta(x, y) => (f64::from(*x), f64::from(*y)),
                    MouseScrollDelta::PixelDelta(position) => {
                        self.scrolled += position.y;
                        let notch = Self::notch(metrics);
                        let notches = (self.scrolled / notch).trunc();
                        self.scrolled -= notches * notch;
                        (position.x / notch, notches)
                    }
                };
                // Positive `y` is *away* from the user in winit, which
                // is a scroll up.
                for _ in 0..dy.abs().round() as u32 {
                    let kind = if dy > 0.0 {
                        MouseKind::ScrollUp
                    } else {
                        MouseKind::ScrollDown
                    };
                    self.mouse(kind, metrics, size, hits, out);
                }
                for _ in 0..dx.abs().round() as u32 {
                    let kind = if dx > 0.0 {
                        MouseKind::ScrollRight
                    } else {
                        MouseKind::ScrollLeft
                    };
                    self.mouse(kind, metrics, size, hits, out);
                }
            }
            WindowEvent::KeyboardInput { event, .. } => {
                let kind = match (event.state, event.repeat) {
                    (ElementState::Pressed, true) => KeyKind::Repeat,
                    (ElementState::Pressed, false) => KeyKind::Press,
                    (ElementState::Released, _) => KeyKind::Release,
                };
                out.extend(self.keyed(&event.logical_key, kind));
            }
            _ => {}
        }
    }

    /// One key report, with the modifiers this input has been told are
    /// held.
    ///
    /// Its own method rather than a branch inside [`Input::fold`]
    /// because winit's `KeyEvent` carries a per-platform field with no
    /// portable constructor — so a test that could only reach the key
    /// path through `fold` could not be written at all on macOS. The
    /// window still only calls it from `fold`.
    #[must_use]
    pub fn keyed(&self, key: &Key, kind: KeyKind) -> Option<Msg> {
        let code = self::key(key)?;
        Some(Msg::Key(KeyEvent::new(code).with(self.mods).kind(kind)))
    }

    /// One mouse report at the pointer's current cell, with the widget
    /// under it already resolved — the same `(event, target)` pair the
    /// terminal runtime folds.
    fn mouse(
        &self,
        kind: MouseKind,
        metrics: &CellMetrics,
        size: Size,
        hits: &HitRegistry,
        out: &mut Vec<Msg>,
    ) {
        let (col, row) = metrics.cell_at(self.at.0, self.at.1, size);
        // The pixel travels with the cell: the cell is the uniform
        // division `Msg::Mouse` always carried, and the pixel is what a
        // drag or a pan reads on a surface whose rows are not all one
        // cell tall (`Model::pointer_at`). Rounded, never floored — a
        // pointer at 8.6 px is nearer column 1's edge than column 0's.
        let px = (
            self.at.0.round().max(0.0) as u32,
            self.at.1.round().max(0.0) as u32,
        );
        out.push(Msg::Mouse {
            event: MouseEvent::new(kind, col, row)
                .with(self.mods)
                .at_px(px.0, px.1),
            // The *target* is resolved in pixels, against the boxes the
            // layout actually placed, so a click names the widget under
            // the pointer even where a row is not a cell tall.
            target: hits.hit_at(self.at.0, self.at.1),
        });
    }
}

/// winit's modifier state as the cockpit's.
fn modifiers(state: ModifiersState) -> Modifiers {
    let mut mods = Modifiers::NONE;
    for (held, bit) in [
        (state.shift_key(), Modifiers::SHIFT),
        (state.alt_key(), Modifiers::ALT),
        (state.control_key(), Modifiers::CTRL),
        (state.super_key(), Modifiers::SUPER),
    ] {
        if held {
            mods = mods | bit;
        }
    }
    mods
}

/// winit's button as the cockpit's. A fourth button has no terminal
/// report and is dropped rather than folded into one of the three.
fn button(button: MouseButton) -> Option<Button> {
    match button {
        MouseButton::Left => Some(Button::Left),
        MouseButton::Middle => Some(Button::Middle),
        MouseButton::Right => Some(Button::Right),
        _ => None,
    }
}

/// winit's logical key as the cockpit's [`KeyCode`].
///
/// The **logical** key, not the physical one: the cockpit's bindings are
/// letters (`n`, `l`, `g`, `?`) and a letter is what a layout produces,
/// not what a scancode is. A key the cockpit has no code for is `None`
/// and reaches `update` as nothing at all, which is what a terminal does
/// with a sequence it cannot decode.
fn key(key: &Key) -> Option<KeyCode> {
    match key {
        Key::Character(text) => text.chars().next().map(KeyCode::Char),
        Key::Named(named) => match named {
            NamedKey::Enter => Some(KeyCode::Enter),
            NamedKey::Tab => Some(KeyCode::Tab),
            NamedKey::Backspace => Some(KeyCode::Backspace),
            NamedKey::Escape => Some(KeyCode::Esc),
            NamedKey::Space => Some(KeyCode::Char(' ')),
            NamedKey::ArrowUp => Some(KeyCode::Up),
            NamedKey::ArrowDown => Some(KeyCode::Down),
            NamedKey::ArrowLeft => Some(KeyCode::Left),
            NamedKey::ArrowRight => Some(KeyCode::Right),
            NamedKey::Home => Some(KeyCode::Home),
            NamedKey::End => Some(KeyCode::End),
            NamedKey::PageUp => Some(KeyCode::PageUp),
            NamedKey::PageDown => Some(KeyCode::PageDown),
            NamedKey::Insert => Some(KeyCode::Insert),
            NamedKey::Delete => Some(KeyCode::Delete),
            NamedKey::F1 => Some(KeyCode::F(1)),
            NamedKey::F2 => Some(KeyCode::F(2)),
            NamedKey::F3 => Some(KeyCode::F(3)),
            NamedKey::F4 => Some(KeyCode::F(4)),
            _ => None,
        },
        _ => None,
    }
}

#[cfg(test)]
#[path = "input.tests.rs"]
mod tests;
